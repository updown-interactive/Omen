# Omen

A Rust-first declarative desktop UI framework inspired by SwiftUI. Phase 1 establishes the workspace and a native GPUI welcome window. The component macro, `view!`, and reactive state are design proposals for Phase 2.

## Setup

Install Rust through [rustup](https://rustup.rs/). The repository selects stable Rust with rustfmt and Clippy. GPUI builds are substantial; allow disk space and time for the initial dependency download and compilation.

On macOS, install full Xcode, select it with `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`, and complete its first-launch setup. Omen enables GPUI’s runtime shader compilation, so the optional Xcode Metal compiler download is not required.

On Ubuntu, install the native dependencies listed in [.github/workflows/ci.yml](.github/workflows/ci.yml). A graphical session and working GPU drivers are required to launch the example; CI compilation does not verify rendering.

```sh
cargo run -p hello-world --locked
cargo run -p omen-cli --locked -- doctor
```

The example opens an 800 × 520 window titled Omen, with “Welcome to Omen” and “A Rust-first UI framework”. Closing its last window exits the application. `doctor` is a Phase 1 CLI smoke check, not a system dependency diagnostic.

## Workspace

| Crate | Responsibility |
| --- | --- |
| omen-core | Renderer-independent contracts, future lifecycle and state interfaces |
| omen-macros | Reserved proc-macro crate for Phase 2 |
| omen-ui | Public components and modifiers, reserved for Phase 2 |
| omen-gpui | GPUI rendering adapter and native window lifecycle |
| omen-cli | `omen doctor`; future project and build tooling |
| hello-world | Minimal desktop entry point |

`omen-core` has no GPUI dependency. There are no implemented component, view, or state macros yet. See [the DSL draft](docs/dsl-spec.md).

## Validation

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## GPUI revision

Both GPUI dependencies use Zed commit `faf8f0b6980205ca2659af174790a212ae3c416a`. Keep their revisions synchronized and commit `Cargo.lock` after dependency updates. Locally verified on macOS with stable Rust 1.98.1: formatting, compilation, Cargo tests (currently zero test cases), Clippy, CLI doctor, and visible welcome text. CI is configured for macOS and Ubuntu; remote CI is not yet verified. See [integration notes](docs/gpui-integration.md) for platform assumptions and verification details.

License: MIT OR Apache-2.0.
