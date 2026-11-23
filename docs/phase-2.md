# Phase 2: declarative compiler

The pipeline is `omen-macros → omen-macro-core` (parsing, AST, validation, registry, generation). Expanded code uses `omen-ui`, whose builders consume into the GPUI-independent `omen-core::ViewNode`. `omen-gpui` retains that tree and translates it to GPUI elements. Neither the parser nor macro crate depends on GPUI.

Run the demonstration:

```sh
cargo run -p declarative-ui --locked
```

The example defines Header and TaskCard views in separate Rust modules, composes them with named properties in main!, and includes a styled button. TaskCard computes its status marker using a local Rust statement. Clicking Get Started prints `Omen is running!` to the launching terminal. The initial Phase 2 compiler constructed a static tree. Named views now use the [reactive runtime](reactive-state.md), with stable structural/keyed identity and dependency-driven rebuilding. Closing the last window quits the event loop.

See [the language overview](dsl-spec.md) and [grammar](omen-grammar.md) for main!, anonymous/named view!, named arguments, local statements, modifiers, ownership rules, and scope limitations.

## Validation

```sh
cargo test -p omen-macro-core --locked
cargo test -p omen-ui --no-default-features --locked
cargo test -p omen-macros --test compile --locked
cargo test -p omen-gpui --lib --locked
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The parser, compile fixtures, and runtime tests with omen-ui default features disabled run without GPUI. Trybuild checks compile-pass fixtures and checked-in compile-fail stderr snapshots. GPUI's test platform draws the tree and injects mouse events, including a click after redraw and a click outside the button. Native desktop inspection separately verifies actual font/shader rendering. CI runs workspace checks on macOS and Ubuntu; remote results are not implied by local validation.

When intentionally changing diagnostics, run `TRYBUILD=overwrite cargo test -p omen-macros --test compile`, inspect the stderr diffs, then rerun without overwrite. Snapshots can change with stable Rust upgrades; review rather than blindly accepting them.

## Validation coverage

Compiler tests cover token-aware parsing, both view! forms, main! expansion, combined diagnostics, named-property handling, and generated Rust syntax. Runtime tests cover branch/row ordering, typed layout modes, module composition, String conversion, local statements, and retained action captures. Trybuild checks both valid programs and diagnostic snapshots for invalid DSL/properties. The GPUI tests cover click dispatch after redraw and horizontal, vertical, and overlapping layout hit testing.

The upstream `block` 0.1.6 future-incompatibility notice remains; it does not fail current checks. Remote CI results are not implied by local validation.

## Current local verification

On macOS with rustup stable 1.98.1, locked workspace compilation, formatting, full workspace tests, and Clippy with warnings denied passed. This includes nine compiler/parser tests, eight runtime tests, two GPUI render/event tests, one rustdoc example, and eight compile-pass plus thirty compile-fail fixtures. Final fixture verification ran without snapshot overwrite.

The migrated declarative-ui executable was launched through main! and visually inspected in an ignored temporary app bundle. Header rendered “My Tasks” and “Stay productive”; the two TaskCard instances rendered their completed/incomplete markers and titles. Both applications now contain only main!/view! UI code, without manual entry points or view implementations.
