# Omen

A Rust-first declarative desktop UI framework inspired by SwiftUI. Omen provides a declarative compiler, native controls, reusable views, and a retained reactive runtime with state, bindings, lifecycle hooks, and cancellable async actions.

## Setup

Install Rust through [rustup](https://rustup.rs/). Ensure its Cargo and Rust tools are first on PATH; if Homebrew Rust takes precedence, use `export PATH="$HOME/.cargo/bin:$PATH"`. The repository selects stable Rust with rustfmt and Clippy. GPUI builds are substantial; allow disk space and time for the initial dependency download and compilation.

On macOS, install full Xcode, select it with `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`, and complete its first-launch setup. Omen enables GPUI’s runtime shader compilation, so the optional Xcode Metal compiler download is not required.

On Ubuntu, install the native dependencies listed in [.github/workflows/ci.yml](.github/workflows/ci.yml). A graphical session and working GPU drivers are required to launch the example; CI compilation does not verify rendering.

```sh
cargo run -p native-plugins --locked
cargo run -p navigation-styling --locked
cargo run -p reactive-state --locked
cargo run -p controls-gallery --locked
cargo run -p declarative-ui --locked
cargo run -p hello-world --locked
cargo run -p omen-cli --locked -- doctor
```

See [the demo source](examples/declarative-ui/src/main.rs) for the complete application. The declarative example opens an 800 × 520 window titled Omen with reusable Header and TaskCard views and a Get Started button. Click it to print `Omen is running!` in the terminal. The original hello-world example remains available. Closing its last window exits the application. `doctor` is a Phase 1 CLI smoke check, not a system dependency diagnostic.

See [the view library guide](docs/view-library.md) for controls, shared values, accessibility, keyboard navigation, appearance modifiers, and reusable views.

See [reactive state and lifecycle](docs/reactive-state.md) for `@State`, `$binding`, environment models, keyed lists, effects, and async actions.

See [navigation, styling, and animation](docs/navigation-styling.md) for route stacks, presentations, tabs, themes, responsive layouts, motion, and localization.

See [native plugins](docs/native-plugins.md) for typed registration, permissions, events, desktop adapters, and Android/iOS/web integration.

## Documentation

- [Documentation index](docs/README.md)
- [Component reference](docs/view-library.md): constructors, defaults, bindings, modifiers, accessibility and platform qualifications
- [Reactive state and lifecycle](docs/reactive-state.md)
- [Navigation, themes and animation](docs/navigation-styling.md)

Build the API reference with `cargo doc -p omen -p omen-ui -p omen-components --no-default-features --no-deps --locked`.

## Writing views

```rust,ignore
use omen::{main, view};

view! {
    pub Header(title: String) {
        Text(title).font(.largeTitle)
    }
}

main! {
    view! {
        Layout { Header(title: "Omen") }.padding(24)
    }
}
```

Use ordinary Rust modules to organize and import views. Named arguments can appear in any order; string literals convert to String properties. Local Rust statements can precede the UI inside named views. Applications need only the omen dependency with its default desktop feature. Framework internals generate the Rust types, view implementations, and native entry point.

Reusable logic can live beside a view or in separate Rust modules: `state!` defines model fields and business methods, `locale!` defines translation catalogs, and `environment!` defines shared models. See [the separated logic example](examples/separated-logic/src/main.rs) and [reactive state documentation](docs/reactive-state.md). Text interpolates values and fields directly, for example `Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}")`.

## Workspace

| Crate | Responsibility |
| --- | --- |
| omen-plugin | Typed native operations, permissions, lifecycle, events, and bridge protocol |
| omen-native | Rooted desktop filesystem, notifications, and optional camera adapters |
| omen-plugin-ffi | C ABI used by Swift and Android JNI |
| omen-plugin-web | JavaScript/WASM native host transport |
| omen-core | Renderer-independent View, owned runtime nodes, styles, actions |
| omen-macros | `main!` startup and anonymous/named `view!` entry points |
| omen-macro-core | Parser, AST, validation, diagnostics, and code generation |
| omen | Public application facade, prelude, main! and view! |
| omen-ui | Content, form, layout, and status views with typed modifiers |
| omen-components | Reusable named views built from the public DSL |
| omen-gpui | GPUI rendering adapter and native window lifecycle |
| omen-cli | `omen doctor`; future project and build tooling |
| hello-world | Original desktop entry point |
| declarative-ui | Named-view task-screen demonstration |
| controls-gallery | Interactive view/control library gallery |
| reactive-state | State, bindings, keyed identity, and cancellable async work |
| navigation-styling | Navigation, presentations, tabs, themes, responsiveness, motion, and RTL |
| native-plugins | Typed filesystem and native event demonstration |

`omen-core` has no GPUI dependency. The compiler supports conditions, loops, eager lists, and action blocks. See [Phase 2](docs/phase-2.md) and [the implemented grammar](docs/omen-grammar.md). Named views support `@State` and `@Environment` declarations, dependency tracking, and lifecycle reconciliation.

## Validation

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## GPUI revision

Both GPUI dependencies use Zed commit `faf8f0b6980205ca2659af174790a212ae3c416a`. Keep their revisions synchronized and commit `Cargo.lock` after dependency updates. Locally verified on macOS with stable Rust 1.98.1: formatting, compilation, parser, runtime, and compile-pass/fail tests, Clippy, CLI doctor, and visible welcome text. CI is configured for macOS and Ubuntu; remote CI is not yet verified. See [integration notes](docs/gpui-integration.md) for platform assumptions and verification details.

License: MIT OR Apache-2.0.

Cross-platform builds, backend status and validation: [Phase 7](docs/cross-platform.md).

Native mobile uses GPUI: Android NativeActivity + Vulkan and iOS UIKit + Metal. The [native mobile counter](examples/native-mobile) demonstrates the shared DSL. Android emulator rendering and touch are verified; iOS simulator launch is still under investigation. See the [platform status](docs/cross-platform.md).

Create and run an app with the CLI:

```sh
cargo install --path crates/omen-cli --locked
omen new my-app
cd my-app
omen run
```

See [CLI setup, devices and configuration](docs/cli.md). Generated projects use this
local SDK checkout; mobile rendering remains native GPUI.
