# GPUI integration

## Revision

Zed commit `faf8f0b6980205ca2659af174790a212ae3c416a`, selected on 2026-11-12 from upstream history (2026-10-22), before the macOS migration that introduced unstable Option::map_or_default calls. Both dependencies use this exact revision.

Sources at the pinned revision:

- [Platform manifest](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/gpui_platform/Cargo.toml)
- [Hello world](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/gpui/examples/hello_world.rs)
- [Example font setup](https://github.com/zed-industries/zed/blob/faf8f0b6980205ca2659af174790a212ae3c416a/crates/gpui/examples/example_support/fonts.rs)

The platform manifest routes macOS to gpui_macos, Windows to gpui_windows, and Linux/FreeBSD to gpui_linux. Omen enables font-kit, Wayland, and X11 as in the Phase 1 instructions, plus runtime_shaders to compile Metal shaders at application startup. The initial default build identified a missing optional Xcode Metal Toolchain; this upstream feature avoids requiring that separate installation. These indicate upstream platform implementations, not completed Omen runtime verification on every platform. Mobile and web are outside this milestone.

Native examples do not add bundled fonts at this revision; the upstream font helper returns true without loading fonts on native targets. Omen follows that pattern and relies on system fonts.

## Architecture

The hello-world executable calls `omen_gpui::run()`. The adapter owns the GPUI App, Window, Context, root entity, and event loop. Future public components belong in omen-ui and renderer-independent contracts belong in omen-core. Phase 1 deliberately leaves their API undefined rather than inventing reactive or identity semantics before Phase 2.

The CLI's doctor command only confirms CLI execution. It does not claim to validate graphics drivers, Rust, or native libraries.

## Verification

Validated locally on macOS with stable Rust 1.98.1:

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace`: passed against the pinned revision.
- `cargo test --workspace --locked`: passed (zero tests; Phase 1 crates have no test cases yet).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo run -p omen-cli --locked -- doctor`: printed the expected environment and CLI messages.
- `cargo run -p hello-world --locked`: built and started successfully. For automated visual inspection, the same executable was wrapped in a temporary app bundle under ignored `target/Omen.app`. Its Omen window and both welcome lines were confirmed in a desktop screenshot.
- `Cargo.lock` records the resolved graph for the final pinned revision.

The latest upstream revision initially tested (`78f5d7446c55f6c1aa9b66e8111f6b3f4625ebb0`) failed on stable due to `Option::map_or_default` usage in gpui_macos. The selected revision predates that change and passed the checks above.

Cargo reports a future-incompatibility notice for upstream `block` 0.1.6. Current checks pass; reassess when upgrading Rust or GPUI.

GitHub Actions is configured for macOS 15 and Ubuntu 24.04. No remote CI run has been observed; Linux compilation and runtime rendering have not been tested locally. A passing CI run is still required before declaring the full Phase 1 acceptance checklist complete.
