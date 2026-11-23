# Omen documentation

Start with the [component reference](view-library.md) for a working application, every built-in control, binding rules, layout, navigation, modifiers and the reusable library.

| Guide | Use it for |
| --- | --- |
| [Component reference](view-library.md) | Choosing and using views, defaults, properties and examples |
| [Reactive state and lifecycle](reactive-state.md) | `state!`, bindings, direct mutations, effects, tasks and logic organization |
| [Navigation, styling and animation](navigation-styling.md) | Route stacks, overlays, tabs, themes, responsive layout, localization and motion |
| [CLI](cli.md) | Creating, running, building and packaging applications |
| [Cross-platform status](cross-platform.md) | Backend capabilities, native mobile integration and actual validation status |
| [Native plugins](native-plugins.md) | Typed native operations, permissions, events and platform bridges |
| [DSL grammar](omen-grammar.md) | Supported macro syntax, compile-time checks and generated Rust |
| [GPUI integration](gpui-integration.md) | Framework renderer integration |

Application code imports from `omen`. The `omen-components` dependency provides the optional reusable library. Runtime, renderer and compiler crates remain separate implementation layers.

Build the generated API reference with:

```sh
cargo doc -p omen -p omen-ui -p omen-components --no-default-features --no-deps --locked
```

Open `target/doc/omen/index.html` for the public facade or `target/doc/omen_components/index.html` for reusable views. These docs include executable API examples; the component guide also contains UI fragments intended to be placed inside `view!`.
