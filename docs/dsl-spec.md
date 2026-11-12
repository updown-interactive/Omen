# Omen UI DSL Specification — Draft v0.1

## Design principles

Omen is a declarative Rust UI framework inspired by SwiftUI.

1. UI components use `Text("Hello")`, not `Text::new("Hello")`.
2. Layout containers use nested blocks such as `VStack { ... }`.
3. Modifiers are chainable: `.padding(16).background(.blue)`.
4. Application code should not require direct GPUI `Window` or `Context` parameters.
5. State should be reactive, with concise declarations and two-way bindings.
6. Rust types, ownership, and compiler safety remain intact after macro expansion.

## Proposed syntax

```
#[component]
fn Counter() -> impl View {
    @State let count = 0;

    view! {
        VStack {
            Text("Count: {count}")
                .font(.title)

            Button("Increment") {
                count += 1;
            }
            .style(.primary)
        }
        .spacing(16)
        .padding(24)
    }
}
```

## Syntax conventions

- `#[component]`: transforms a component function.
- `view!`: parses a declarative UI tree.
- `@State`: declares local reactive state inside a component macro.
- `$value`: references a two-way binding.
- `.modifier(...)`: transforms a view or configures its behavior.
- `if` and `for`: provide conditional and repeated UI.
- `Button("Title") { ... }`: declares an event action.

## Compiler requirements

- Produce valid Rust from the DSL.
- Preserve Rust type checking.
- Report meaningful errors at the original source locations.
- Avoid implicit global mutable state.
- Preserve component identity across rerenders.
- Support future cross-platform rendering backends.

## Phase 1 scope

The DSL is a documented design only. The initial milestone is a working GPUI desktop window and an extensible workspace architecture.

The syntax above is intentionally illustrative: `@State`, `.title`, and `$value` require custom parsing or transformation. They cannot be used as ordinary Rust expressions.

