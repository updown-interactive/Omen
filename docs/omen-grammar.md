# Omen grammar — Phase 2

```text
MacroInput    := NamedView | ViewTree
NamedView     := Visibility? Identifier ("(" PropertyList? ")")?
                 "{" (RustStatement | StateDecl | EnvironmentDecl | LocalizeDecl)* ViewTree "}"
Property      := Identifier ":" RustType
ViewTree      := View+
View          := Element | Conditional | Loop
Element       := Identifier Arguments? Content? Modifier*
Arguments     := "(" ArgumentList? ")"
Argument      := RustExpression | Identifier ":" RustExpression
Content       := ChildBlock | ActionBlock | ListBlock
ChildBlock    := "{" View* "}"
ActionBlock   := RustBlock
ListBlock     := "{" RustPattern "in" View* "}"
Modifier      := "." Identifier "(" ModifierArgumentList? ")"
ModifierArg   := RustExpression | "." Identifier
Conditional   := "if" RustCondition "{" View* "}"
                 ("else" ("{" View* "}" | Conditional))?
Loop          := "for" RustPattern "in" RustExpression "{" View* "}"
```

The grammar is token based, implemented with `syn::ParseStream`. Parenthesized arguments are comma-separated Rust expressions, with optional trailing commas. Modifiers use parentheses for arguments and braces for action or presentation content. Most take one argument; frame takes two and gradient takes three. Sibling views are adjacent, without commas or semicolons. Leading Rust statements are accepted inside named view declarations; ordinary layout child blocks contain views only. Button action blocks contain Rust statements. Parenthesize struct literals used in conditions or collection expressions, just as Rust requires before a block.

`Button` blocks parse as Rust blocks. `Layout` and `Card` blocks parse as view trees. `List(items)` requires `{ pattern in ... }`; patterns support destructuring. `Text` cannot have a child block. Constructors require one argument for Text, Button, and List, and zero for Layout, Card, and Divider. Empty containers and empty branches/rows are permitted; an entirely empty `view!` is rejected.

Conditions include ordinary boolean expressions, `if let`, and `else if`. Conditions and collections retain Rust evaluation and ownership rules. For loops accept Rust patterns and any IntoIterator expression. No string splitting or custom expression evaluator is used.

## Registered modifiers

| DSL name | Accepted value | Runtime operation |
| --- | --- | --- |
| mode | `.horizontal`, `.vertical`, `.stack` or `LayoutMode` expression | Layout arrangement |
| font | `.body`, `.title`, `.largeTitle` or `Font` expression | semantic font size |
| weight | `.regular`, `.medium`, `.bold` or `Weight` expression | font weight |
| color / background | `.white`, `.black`, `.blue`, `.secondary`, `.green`, `.dark` or `Color` expression | foreground / background |
| padding | numeric Rust expression convertible to f64 | uniform logical pixels |
| spacing | numeric Rust expression convertible to f64 | container gap |
| cornerRadius | numeric Rust expression convertible to f64 | corner radius |

`spacing` is accepted on Layout, Card, and List. Unknown names, wrong arity, and symbols from the wrong domain are compile errors. Non-symbolic expressions retain Rust type checking. Later modifiers of the same kind replace earlier values. Styling uses explicit overrides, with inherited text styles where GPUI supports them; ordered wrapper-style modifier semantics are not implemented.

## Expansion and ownership

Applications import `omen::{main, view}`. `main! { HomeScreen() }` generates a Rust main function and starts the GPUI adapter. The desktop backend is a default omen feature; library consumers can opt out with `default-features = false` when they only need view construction. `main!` requires the desktop feature.

`view! { pub Header(title: String) { Text(title) } }` declares a reusable Rust type with a generated View implementation. Visibility can be omitted or use ordinary Rust forms such as `pub(crate)`. `Header(title: "Hello")` constructs that view; named properties may appear in any order. Zero-property views use `HomeScreen()`. Custom view names resolve through normal Rust imports, including cross-module reexports. Named views cannot replace reserved built-in names. Unknown names and missing/unknown/type-incompatible properties are checked by Rust; duplicate properties/arguments, mixed argument styles, and named arguments on built-ins are diagnosed by Omen. Reusable views accept named arguments and do not accept a trailing child block.

Leading local Rust statements execute each time the named view rebuilds. They may compute text, iterate data, import helpers, or perform ordinary Rust work. `@State let name[: Type] = expression;` initializes a persistent slot once. `@Environment let name: Type;` resolves a shared model. A nonempty UI tree follows the declarations. Generic view declarations and inferred property types are not implemented; declared property types are ordinary explicit Rust types.

Expansion resolves the consumer’s `omen` dependency, including Cargo dependency aliases. The legacy `omen-ui` crate remains supported. Named arguments lower to typed struct fields with Into conversions, with no runtime reflection or argument-name lookup. String literals convert to owned String properties; numeric values must satisfy Rust's normal Into bounds (use an explicit numeric type when needed). Generated properties are implementation details; application code uses named DSL calls. Generated temporary identifiers use mixed-site hygiene. Traits used by expansion are fully qualified or imported locally. There is no separate reusable-object category or attribute macro.

`View::into_node` consumes a builder into an owned tree. Static constructors remain typed; AnyView is the common node wrapper for branch and modifier results, not a boxed `dyn View`. Text owns its String. Children are Vec-backed nodes. Button actions become `move || -> () { ... }` closures with `Fn() + 'static` bounds, stored in `Rc`; clone an Rc before moving it into multiple actions. Borrowed local captures, consuming a captured value, and mutating captures directly remain Rust errors. Interior mutability can be used explicitly for event effects, without automatic rerendering.

Conditions select only their active branch. For loops eagerly append sibling nodes. Basic List eagerly builds a distinct List node containing row groups, and each row gets a vertical layout. No stable item identity, diffing, or virtualization is promised. A future keyed API must define identity and duplicate-key behavior before reconciliation is introduced; the proposed `id: \\.id` syntax is not parsed now.

## Diagnostics and scope

Parsing errors stop at the malformed token; semantic validation combines independent errors across the tree. Macro entry points emit expression-safe compile-error blocks and preserve user spans. Compiler AST types are private to omen-macro-core and distinct from runtime omen-core nodes.

The reactive extension supports `@State`, `$bindings`, state captures in Text literals, automatic rerendering, and lifecycle hooks. Text literals interpolate local values, state and fields directly, including format specifiers such as `{count:03}`; see [reactive state and lifecycle](reactive-state.md).

## Unified Layout

```rust,ignore
Layout {
    Text("First")
    Text("Second")
}.mode(.horizontal).spacing(12)
```

`Layout` replaces VStack, HStack, and ZStack. `.mode(.vertical)` (the default) arranges children in a column, `.mode(.horizontal)` arranges them in a row, and `.mode(.stack)` overlays them in the same grid cell. Stack size includes the largest child on each axis; later children paint above earlier children and receive overlapping pointer events first. Control-flow groups remain transparent in every mode. Spacing separates children in horizontal/vertical modes; it has no effect on overlaid children in stack mode. The mode modifier is accepted only on Layout, not on Text, Button, or List. The Rust API also accepts `LayoutMode::Horizontal`, `LayoutMode::Vertical`, and `LayoutMode::Stack`. Legacy stack component names are rejected.


The complete built-in view and modifier inventory now lives in [the view library guide](view-library.md). Form controls are single-line and accept explicit handles or `$state` / `$model.field` bindings. frame takes two Rust expressions; gradient takes two color arguments and one numeric angle. The `effect` modifier takes a dependency and cleanup-producing closure. Other parenthesized modifiers take one argument; lifecycle and action modifiers also accept Rust blocks. The compiler validates modifier arity, symbol domains, and control-specific value/checked/onChange applicability.

See [reactive state and lifecycle](reactive-state.md) for declaration semantics, keyed `List(items, id: |item| item.id)`, readable `.onTap { ... }`, async actions, and retained identity. `state!` is rejected; no additional public macro is required.

Phase 5 adds `NavigationStack(navigator, destination) { root }`, `NavigationBar(title) { actions }`, `TabView(binding) { tabs }`, and `Tab(id, title) { content }`. Presentation modifiers take a binding and view factory. The [navigation and styling guide](navigation-styling.md) documents theme/token symbols, responsive overrides, safe areas, entry transitions, easing, spring timing, localization, and logical RTL layout.

## Compact application syntax

Use the public `omen` crate (`use omen::{main, view};` or `use omen::prelude::*;`).

```rust
use omen::prelude::*;

view! {
    Home {
        @State let count = 0
        @State let sheet = false
        @Environment viewport
        Layout {
            Text("Count: {count}")
            Button("Increment") { count += 1 }
            Button("Show sheet") { sheet = true }
        }
        .spacing(.md)
        .padding(.lg)
        .horizontal(.medium)
        .sheet($sheet) { Text("Hello from the sheet") }
        .animation(.spring(800.ms))
    }
}
main! { Home() }
```

Zero-property declarations can omit parentheses. State declarations use `@State let`; the trailing semicolon is optional. Inferred string literals create owned strings. The shorter spelling without `let` remains accepted for compatibility. Anonymous `view!` roots support the same declarations. `@Environment viewport` and `@Environment lifecycle` infer their framework types; custom environment models still require an explicit type.

`padding` and `spacing` accept numeric values or semantic `.sm`, `.md`, `.lg`, and `.xl` tokens. `.horizontal(.medium)` switches arrangement at that breakpoint and above. Presentation content follows `.sheet($visible)`, `.dialog`, `.popover`, or `.modal` directly in braces. Existing closures and token-specific modifiers remain supported.

A state initialized with `Navigator()` can drive `NavigationStack($navigator)`. Inside an action, `navigator.push(Detail(title: "Detail"))` registers a typed destination automatically. Each push has a unique identity; popping disposes that visit while retaining earlier screens' state. The legacy string-route constructor and factory remain available.

Inline translations use `@Localize strings { en { welcome: "Welcome" } ar { welcome: "مرحبا" } }`. Read `strings.welcome`, write `strings.locale = "ar"`, and apply `.locale(strings.locale)` to update text and layout direction. The first declared locale is the fallback.

Animation shorthand `.spring(800.ms)` or `.spring(1.s)` uses the default spring parameters. `.offset(x: value, y: value)` defaults omitted axes to zero. Conditions remain Rust expressions: `.theme(if dark { .dark } else { .light })`. Rust’s ownership rules still apply to ordinary values, so clone an owned property when consuming it more than once.

### State models

`state! { pub Model { count = 0 enabled = true pub fn increment(&mut self) { self.count += 1; } } }` defines an ordinary model. Fields accept optional `let`, annotations, and semicolons. `@State model = Model()` registers the initialized model with the existing view state runtime. In action blocks `model.increment()` lowers to an observable update; `$model.field` projects the same storage. See [reactive state](reactive-state.md#compact-state-declarations-and-reactive-actions) for inference rules and mutation diagnostics.
