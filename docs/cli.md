# Omen CLI v0.1

`omen` is the standard entry point for creating, building and running applications.
The CLI invokes the shared `scripts/package.py` pipeline; Android uses native
GPUI/Vulkan, iOS uses GPUI/UIKit/Metal and browsers use Rust/WASM + DOM.

## Install from this checkout

Install a Rust toolchain and keep this SDK checkout available. From its root:

```sh
cargo install --path crates/omen-cli --locked
omen --help
omen doctor
```

Make sure Cargo's `bin` directory is on PATH. During framework development you can
use `cargo run -p omen-cli -- <command>` instead. This SDK is not yet published as
an `omen-ui` registry release: generated Cargo manifests reference the local
checkout. `omen new --sdk /path/to/Omen` or `OMEN_SDK` selects the SDK used for new
projects. If the SDK moves, update `[toolchain].sdk` in existing projects.

```sh
omen new my-store
cd my-store
omen run
```

The starter contains `Cargo.toml`, `Omen.toml`, `src/main.rs`, `src/views/mod.rs`,
`src/views/home_view.rs`, `assets/` and `.gitignore`. The starter Home view has an
`@Design` marker for Studio. `src/main.rs` is the single application entry file
on every platform. Cargo builds that same source as a native executable, an Android
shared library or a WASM application. `main!` generates the platform entry points.
Application UI uses only `main!` and
`view!`; no handwritten `impl View` or component macro is needed. The generated
Cargo project is isolated from any enclosing workspace. Existing directories are
never overwritten.

## Run and build

```sh
omen run                       # Arrow-key platform/device selection; Esc cancels
omen devices                   # Connected Android devices, AVDs and iOS simulators
omen run -p desktop
omen run -p android --device emulator-5554
omen run -p android --device avd:Pixel_7
omen run -p android --device emulator-5554 --no-logs
omen run -p web
omen run -p web --no-open --port 8080
omen run -p ios --device "iPhone 17 Pro"
omen build -p android
omen build -p desktop --release
omen build -p ios
```

`--platform` and `-p` are equivalent. `--project /path/to/app` works before or after
the subcommand; project discovery also searches parent directories. In scripts,
supply `--platform` and an unambiguous `--device` when needed. The CLI refuses to
wait for interactive input without a terminal. Device IDs are preferable when
names occur in more than one simulator runtime. JSON discovery is available with
`omen doctor --json` and `omen devices --json`.

Unavailable toolchains are displayed but excluded from interactive selection.
Availability means tools were detected, not that rendering was verified. In
particular, iOS always reports **build available, launch unverified** when its
build prerequisites are present. `doctor` does not install tools or mark a
platform Ready merely because its SDK exists.

Android selects an authorized connected device or a named installed AVD. Multiple
choices require selection; unauthorized/offline devices cannot be selected. AVD
selection starts the existing emulator without wiping its data. The initial native
APK supports **ARM64 only** and needs a Vulkan-capable Android device, minimum API
26. The CLI checks ABI, installs the APK, starts the configured Activity, waits for
GPUI's first presented Vulkan frame and then streams process logcat. `--no-logs`
returns after frame verification. An Activity opening alone is not treated as a
successful renderer launch. Android IME/accessibility limitations still apply;
see [native platform status](cross-platform.md).

The web runner serves on loopback with an automatically chosen port (or `--port`),
uses the correct WASM/JavaScript MIME types and opens the system browser. The
server remains running until Ctrl-C. `--no-open` supports headless servers and CI.
Requests cannot traverse outside the packaged directory, including through
symlinks. Opening a browser is not proof that an arbitrary app rendered correctly.

The iOS runner supports **simulators**, starts a selected shutdown simulator,
installs the native app and attempts launch with bounded timeouts. Failures
propagate as nonzero exits with diagnostics; success reports a process ID without
claiming rendered output. Logs go in `.omen/logs/`. It neither resets nor deletes
your simulator. Physical-device install/launch is not implemented in v0.1.
Device **builds** are available using `omen build -p ios --ios-device --sign`.

## Configuration

```toml
[app]
name = "my-store"
id = "com.example.mystore"
version = "0.1.0"

[window]
title = "My Store"
width = 1100
height = 720

[android]
renderer = "native"

[ios]
renderer = "native"

[web]
renderer = "dom"

[toolchain]
sdk = "/path/to/Omen"
```

Names, dotted application IDs, numeric versions, renderer choices and window
sizes are validated before packaging. Desktop window title and dimensions are
baked into the GPUI build and can be overridden with `OMEN_WINDOW_TITLE`,
`OMEN_WINDOW_WIDTH` and `OMEN_WINDOW_HEIGHT` at launch. Mobile window dimensions
are controlled by the operating system. App identity is applied to native GPUI
and bundle metadata. Android's Kotlin namespace/Activity class stays
`dev.omen.app.OmenActivity`; its application ID and visible label use your config.

`app.package` optionally chooses a Cargo package in a workspace;
`app.native_package` can choose a separate native example. The SDK's existing
`omen.app.json` is supported for running directly from this checkout.

### App icon

Put a square PNG at `assets/icon.png`, or set a project-relative path in
`Omen.toml`. A 1024 × 1024 source provides full resolution for every target.

```toml
[app]
# Existing name, id, and version go in this same section.
icon = "assets/branding/logo.png"
```

`omen build` and `omen run` generate and embed the icons during packaging.
To generate files without compiling or launching the app, use:

```sh
omen generate-icon                       # All platforms enabled in [targets].platforms
omen generate-icon --platform desktop    # A single enabled platform
```

The command writes to `.omen/icons/<platform>/`. Desktop output includes macOS
ICNS, Windows ICO, and a Linux PNG. The macOS ICNS insets and rounds full-bleed
artwork for the Dock and preserves artwork with transparent corners. Android gets
launcher mipmaps, iOS gets an AppIcon asset catalog, and Web gets favicon and touch
icon PNGs. The iOS catalog uses an opaque white background for transparent source
pixels.

## SDK prerequisites and outputs

`omen doctor` checks the current Rust compiler's installed targets, Python 3.9+,
native desktop SDK/libraries, Android SDK/NDK/JDK/Gradle, iOS SDK and WASM target.
Typical Rust target setup:

```sh
rustup target add aarch64-linux-android wasm32-unknown-unknown
# Apple Silicon macOS + Xcode:
rustup target add aarch64-apple-ios-sim
```

Android detection respects `ANDROID_HOME` / `ANDROID_SDK_ROOT`,
`ANDROID_NDK_HOME`, `JAVA_HOME` and `GRADLE`, with standard local SDK/Android Studio
JDK and cached Gradle fallbacks. Default NDK: 28.2.13676358. Install Gradle or point
`GRADLE` to its executable. Detection does not download SDKs automatically.

The CLI passes the project root, config and Cargo target directory to the shared
packager. Packaging therefore works outside the framework repository. Android
staging files live in the project's ignored `.omen/android`, leaving SDK source
unchanged. Cargo's configured target directory and `CARGO_TARGET_DIR` are
respected. Artifacts are normally under `target/omen/packages/<platform>`; the
printed path and `omen-artifact.json` are authoritative. Assets are copied into
the web bundle or native resource folders before signing/checksum generation.

Debug desktop builds and the native mobile development profile are the default;
web output is optimized. `--release` selects release native builds. Android
release packaging requires `OMEN_ANDROID_KEYSTORE`, `OMEN_ANDROID_STORE_PASSWORD`,
`OMEN_ANDROID_KEY_ALIAS` and `OMEN_ANDROID_KEY_PASSWORD`. iOS device signing requires
`OMEN_APPLE_TEAM`, `OMEN_IOS_SIGN_IDENTITY`, `OMEN_IOS_PROVISIONING_PROFILE` and the
`aarch64-apple-ios` Rust target. Desktop signing uses `OMEN_MAC_SIGN_IDENTITY` or
`OMEN_WINDOWS_CERT_THUMBPRINT` with `--sign`. No command publishes or uploads apps.

## Validation

```sh
cargo test -p omen-cli --locked
cargo clippy -p omen-cli --all-targets --locked -- -D warnings
```

Tests cover scaffolding/no overwrite, invalid configuration, noninteractive
behavior, Android device states, simulator availability, web path confinement,
external-project packager protocol and iOS launch-error propagation. Local end-to-end
checks built an external starter for macOS, Android, iOS simulator and WASM;
Android `omen run` installed/launched it and verified native Vulkan presentation.
The web runner served the starter in Chrome with working WASM MIME and no page
errors. iOS runtime verification remains unresolved in the underlying SDK.

### Rustup and package-manager Rust on the same machine

Omen resolves rustup's active toolchain in the selected project directory and uses
its compiler and Cargo consistently for target discovery, metadata and packaging.
This honors `rust-toolchain.toml`, rustup overrides and `RUSTUP_TOOLCHAIN` even when
Homebrew or another system Rust appears earlier on PATH. Your shell configuration
is not modified. Without an available rustup toolchain, Omen retains PATH-based
Rust; an explicit `RUSTC` override also bypasses this selection.

Connected Android phone names are included in the platform menu and doctor output.
If a selected toolchain really lacks the Android target, the phone remains visible
alongside the missing requirement; Omen does not claim it can build. `omen devices`
still lists unauthorized/offline devices with their actual states.

### Enabled project targets

`Omen.toml` may restrict build and run choices:

```toml
[targets]
platforms = ["desktop", "android", "ios", "web"]
```

At least one unique platform is required. Desktop covers the host macOS, Windows,
or Linux pipeline. Older manifests without `[targets]` keep all four targets
enabled. `omen run` lists enabled targets; explicit build/run requests for disabled
targets fail. Studio's new-project dialog edits this manifest before generation.
