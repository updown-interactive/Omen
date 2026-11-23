# Cross-platform backends

Mobile rendering is native GPUI. Android and iOS packaging no longer embeds a
WebView, JavaScript host, DOM renderer or WASM UI bundle. The web target retains
its independent Rust/WASM + semantic DOM backend.

| Platform | Rendering path | Current evidence |
| --- | --- | --- |
| macOS | Omen → GPUI → Metal | Desktop compilation and workspace tests from Phase 7 |
| Linux / Windows | Omen → GPUI → pinned desktop graphics backend | CI matrix configured; local runtime qualification pending |
| iOS | Omen → GPUI iOS → UIKit/CAMetalLayer/Metal | Native simulator executable built and ad-hoc signed; launch still unverified |
| Android | Omen → `gpui_android` → GPUI WGPU renderer → Vulkan/ANativeWindow | Native ARM64 APK built; Vulkan first-frame test passed on API 36 emulator; counter tap verified |
| Web | Omen → Rust/WASM → semantic DOM | Nine browser tests passed before this native migration |

Android has been exercised in the API 36 ARM64 emulator: GPUI rendered the native
view, and a touch changed the Rust counter from 0 to 1. The native Vulkan first-frame
instrumentation test passed. Physical-device and full mobile qualification remain
pending. iOS builds and signs, but simulator launch timed out after 90 seconds;
rendering has not been verified. Prior WebView tests do not validate these backends.
Native mobile is experimental.

## Architecture

`main!`, `view!`, reactive state, composition and styling stay in Omen. The
`omen-gpui` adapter translates views to GPUI elements. The Android platform crate
uses only GPUI platform traits and GPUI's existing scene renderer/text system;
it has no dependency on Omen and contains no Omen view behavior. It is maintained
in the workspace without modifying Cargo's GPUI checkout. It can be developed as
an upstream GPUI platform backend independently of Omen.

The pinned [GPUI iOS backend](https://github.com/zed-industries/zed/tree/faf8f0b6980205ca2659af174790a212ae3c416a/crates/gpui_ios)
already owns UIApplication, scene delegates, Metal surfaces, touch, UIKit text
input and lifecycle integration. Omen now uses that implementation directly.
It is experimental and its accessibility/text behavior still requires testing.

`crates/gpui-android` implements the initial Android backend:

- NativeActivity app-thread event loop and foreground/background task dispatch.
- Vulkan-only adapter/surface initialization using GPUI's WGPU renderer.
- GPUI scene and glyph rendering using Android system fonts and Cosmic Text.
- Surface destruction/recreation that preserves GPU resources and the glyph atlas.
- Density conversion, window resize and NativeActivity content-rectangle insets.
- Multitouch delivery with fresh GPUI identities when Android reuses pointer IDs.
- Visibility, focus, application lifecycle and memory-warning callbacks.
- System Back callback plumbing and a first-presented-frame diagnostic for tests.

It currently supports one activity/window. GPU debug utilities are disabled by
default after the emulator driver crashed in Vulkan debug-object naming; the
platform crate exposes an opt-in `gpu-validation` feature for diagnostics. Android IME composition, hardware text
editing, accessibility tree export, clipboard, credential storage, file pickers,
refresh-rate-aware frame pacing, device-loss recovery and native plugin attachment
are unfinished. Unsupported service methods return errors where the trait permits;
capabilities must not be inferred from trait presence. Content rectangle insets
combine system/keyboard occlusion; they are not classified into separate IME and
safe-area sources yet. Background effects retain the Omen lifecycle behavior, but
state is not persisted across process death. GPU memory-pressure policy and
production-grade task scheduling remain future work.

## Native demo and builds

`examples/native-mobile` is a small Omen view with text and a reactive tap counter.
It avoids presenting unfinished Android text-input or plugin capabilities as
working controls. Both mobile packages use that same view source.

```sh
# Desktop preview of the same native view:
cargo run -p native-mobile

# Android: ARM64, API 26+, Vulkan-capable device, Android SDK/NDK and JDK.
rustup target add aarch64-linux-android
python3 scripts/package.py android

# iOS simulator: Apple Silicon macOS + Xcode.
rustup target add aarch64-apple-ios-sim
python3 scripts/package.py ios
```

Set `ANDROID_HOME`, optionally `ANDROID_NDK_HOME` (default NDK 28.2.13676358),
`JAVA_HOME` and `GRADLE` for Android builds. Outputs are
`target/packages/android/Omen.apk` and `target/packages/ios/OmenApp.app`.

Install Android with `adb install -r target/packages/android/Omen.apk` and launch
`adb shell am start -n dev.omen.platformdemo/dev.omen.app.OmenActivity`.
Use `adb logcat -s gpui_android` to check for `GPUI Vulkan frame presented`.
A launcher activity opening by itself is not a rendering test.

`omen.app.json` selects `native_package` for mobile and `package` for browser and
desktop packaging. Native Android's Rust cdylib target is named `omen_app`, matching
the manifest's `android.app.lib_name`. Android identifiers/names remain configured
in the Gradle template. iOS metadata uses the JSON configuration. `native` selects
GPUI in `omen-ui`; `desktop` remains a compatibility feature alias.

Android release signing uses `android --release` and
`OMEN_ANDROID_KEYSTORE`, `OMEN_ANDROID_STORE_PASSWORD`, `OMEN_ANDROID_KEY_ALIAS`,
`OMEN_ANDROID_KEY_PASSWORD`. iOS device packaging uses `ios --device --sign`, the
`aarch64-apple-ios` Rust target, `OMEN_APPLE_TEAM`, `OMEN_IOS_SIGN_IDENTITY` and
`OMEN_IOS_PROVISIONING_PROFILE`; the profile's application identifier is checked.
Simulator packages use ad-hoc signing. These commands do not obtain certificates,
publish, notarize or submit to stores.

The old WebView prototype source is archived in `platforms/legacy-webview` and is
not compiled by either native mobile package. Its XCTest/DOM tests are historical.
The Phase 6 Kotlin/Swift plugin adapters remain available separately, but have not
yet been attached to these native GPUI application loops.

## Browser and desktop builds

```sh
rustup target add wasm32-unknown-unknown
python3 scripts/package.py web
python3 -m http.server 8000 --directory target/packages/web
python3 scripts/package.py desktop
```

Open `http://localhost:8000`; do not open the HTML directly. Browser builds provide
keyed semantic controls, composition-aware input, focus restoration, lifecycle,
touch and ARIA support. Storage requests probe access before granting it. They
remain distinct from native mobile. Desktop signing uses
`OMEN_MAC_SIGN_IDENTITY` or `OMEN_WINDOWS_CERT_THUMBPRINT` with `desktop --sign`;
Linux produces an unsigned tarball. Windows rooted filesystem access currently
returns Unsupported.

## Validation and remaining milestones

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check -p native-mobile --target aarch64-apple-ios-sim --locked
cargo clippy -p native-mobile --lib --target aarch64-linux-android --locked -- -D warnings
node --test platforms/web/host.test.mjs
```

After native packaging, `scripts/ios-smoke.py` creates its own simulator, installs
the native app, checks that it stays running and captures a screenshot for review.
It deletes only its owned simulator. `scripts/android-smoke.py` creates an owned
ARM64 AVD and runs instrumentation that requires GPUI's first Vulkan frame marker.
Neither script replaces pixel review, input testing or real-device validation.

Development order remains:

1. Qualify the native GPUI iOS view on simulator and iPhone.
2. Qualify Android rectangle/text rendering, touch and surface recreation on a device.
3. Implement Android IME, accessibility, hardware keyboard, frame scheduling and GPU recovery.
4. Connect native plugin hosts, navigation/back handling and full platform behavior.
5. Add rendering comparisons and performance benchmarks. No WebView-vs-native
   benchmark results or performance claims are available yet.

Local native migration checks: the Android target passes Clippy with warnings
denied; desktop `omen-gpui` compilation passes; all nine macro-core tests and two
Android touch-identity tests pass. The Android APK was inspected for native entry
points and absence of WASM/JavaScript UI assets, and its ELF segments use 16 KB
alignment. `target/android-native-ready.png` captures the native counter at 1
after touch. Simulator test processes/devices created for this work were cleaned up.
