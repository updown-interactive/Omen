# Omen UI language

Omen has two public macros:

- `main!` starts the native application with a view expression.
- `view!` constructs an anonymous view or declares one or more reusable named views.

All UI is a view. Application code does not write view trait implementations, render contexts, or application entry-point functions.

```rust,ignore
use omen::{main, view};

view! {
    pub Header(title: String) {
        let heading = format!("Hello, {title}");
        Text(heading).font(.largeTitle)
    }
}

main! {
    view! {
        Layout {
            Header(title: "Omen")
            Text("Views compose views")
        }.spacing(16).padding(24)
    }
}
```

Named views use ordinary Rust module visibility and imports. Property arguments use names, may appear in any order, and are converted into their declared types. String literals can therefore initialize String properties. Rust checks missing/unknown properties and incompatible types. Built-in Text, Button, and List retain positional arguments; Layout takes a child block and a mode modifier.

Several named declarations can share one `view!` block. Place `@Design`
immediately before a named declaration to set Studio’s default preview for the
file and enable Design and Split. View files without a marker open in Code.
Replace existing `@Preview` markers with `@Design`.
Application entry points and navigation continue to use the views named
in application code.

Local Rust statements can precede the UI tree inside a named view. Button blocks are ordinary Rust callbacks. See [the grammar](omen-grammar.md) for exact syntax and ownership behavior.

Named views support `@State`, `@Environment`, `$binding`, automatic dependency invalidation, lifecycle hooks, and cancellable async actions. Text literals can interpolate declared state captures; `format!` supports other computed text. See [reactive state and lifecycle](reactive-state.md). `state!` is rejected to keep the public macro language to `main!` and `view!`.


## Hex colors

`color` (also spelled `textColor` or `textcolor`), `background`,
`hoverBackground`, and the two color stops of `gradient` accept theme tokens
and hex strings. Hex strings accept an optional `#` and use RGB, RGBA,
RRGGBB, or RRGGBBAA order.

```rust,ignore
view! {
    Layout {
        Text("Omen")
            .textColor("#F5F7FF")
            .background("#18233B")
            .hoverBackground("#243455")
        Text("Gradient").gradient(.accent, "#18233BC0", 45)
    }
}
```

Use `Color::from_hex` to validate a string received from user input or a data
source. Invalid strings passed directly to the Rust modifiers resolve to
transparent. The DSL reports invalid string literals at compile time.
