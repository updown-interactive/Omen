# Native plugin architecture

The native plugin layer is independent of GPUI and adds no application macros. `omen-plugin` provides typed contracts and a permission-gated registry; `omen-native` provides desktop implementations; `omen-plugin-ffi` provides a C ABI for Swift and JNI; `omen-plugin-web` provides WASM bindings. Kotlin, Swift, and JavaScript hosts live under `platforms/`.

## Typed registration and calls

```rust
use omen_plugin::{Operation, TypedPlugin, PluginRuntime, PermissionStore, PermissionStatus};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct Echo { text: String }
impl Operation for Echo {
    type Output = String;
    const PLUGIN: &'static str = "example.echo";
    const NAME: &'static str = "echo";
    const PERMISSIONS: &'static [&'static str] = &["example.use"];
}
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let permissions = PermissionStore::default();
permissions.set("example.use", PermissionStatus::Granted); // Trusted host decision.
let runtime = PluginRuntime::new(["example.use"], Arc::new(permissions));
let plugin = TypedPlugin::new("example.echo", "1.0.0")
    .operation::<Echo, _, _>(|request, context| async move {
        context.require("example.use")?;
        Ok(request.text)
    })?;
runtime.register(plugin)?;
futures::executor::block_on(async {
    runtime.start("example.echo").await?;
    assert_eq!(runtime.call(Echo { text: "Hello".into() }).await?, "Hello");
    runtime.shutdown().await
})?;
# Ok(()) }
```

`Operation` associates the request with an output type, plugin ID, operation name, and permissions. `TypedPlugin` serializes and validates the dispatch boundary. Native resource owners can implement the object-safe `Plugin` trait directly: `descriptor`, `start`, `invoke`, and `shutdown`. Registration rejects duplicate identities/capabilities, invalid identifiers, and incompatible API majors.

`discover()` reports descriptors and lifecycle state, including each operation's availability and permissions. Availability does not imply permission. A platform without a camera or notification service should advertise that capability as unavailable rather than returning fabricated success.

The shared operation schemas are `omen_plugin::standard::{ReadText, WriteText, FileChanged, ShowNotification, CapturePhoto, CameraImage}`. `omen-native` reexports them for convenience. WASM and mobile application code can use the schemas without depending on desktop adapters.

## Permissions and trust boundaries

A call requires all of:

1. A registered, Ready plugin and a declared, available operation.
2. Required permissions within the application's immutable allowlist.
3. A Granted result from the host `PermissionBroker`.
4. Native-side checks immediately before protected operations.

New registries and the default `PermissionStore` deny access. `request_permission` is explicit; calls do not silently request permission. A custom broker can marshal a user action to its host and return Prompt, Granted, or Denied. Native hosts independently check their allowlist and current OS/browser grants. Synchronize explicit host permission results into the Rust broker; Rust grant state is not a replacement for OS authorization.

`PermissionStore::set` is a trusted application API, not an operation available to plugins over the wire. Each operation context can recheck only permissions declared for that operation. Registration does not grant filesystem access, send notifications, or activate cameras. Do not derive grants from untrusted plugin metadata.

These are trusted in-process plugins, not a sandbox for hostile Rust, Kotlin, Swift, or JavaScript. Native code can bypass a framework API; use a separate process and an OS sandbox if running untrusted code. Never expose the bridge directly to untrusted web content. A capability descriptor supplied by a host must be reviewed by the embedding application.

## Async work, cancellation, events, and lifecycle

A `PluginCall<T>` is a Send future. Native work begins when polled. Await it on an executor, cancel explicitly with `cancel()`, or drop it to cancel. Runtime shutdown aborts active calls; bridge cancellation sends a best-effort `cancel` message. Hosts suppress stale callbacks. Synchronous work already performed is not rolled back, and OS-owned permission prompts or external camera activities may outlive cancellation. Host code must release resources when results eventually arrive. Applications own operation deadlines; a host that never replies remains pending until cancelled or detached.

Runtime operation and bridge queues limit concurrent pending calls to 256. Bridge frames and encoded operation responses are capped at one MiB. Do not send unbounded files or video frames through JSON; use reviewed resource handles/streaming in a future extension.

`PluginEvent` defines a typed topic. Declare it with `.event::<E>()`, publish with `context.emit(&event)`, and subscribe with `runtime.subscribe::<E>(capacity)`. The returned stream unsubscribes on drop. Capacities must be 1 through 4096; slow consumers receive a terminal Lagged error rather than allowing unbounded buffering. Permission revocation terminates a subscription with a permission error when polled. Plugin stop closes its subscriptions and discards queued events. Each plugin context can emit only its declared topics. Bridge events pass through the same checks.

Lifecycle is Registered → Starting → Ready → Stopping → Stopped. Failed/cancelled initialization becomes Failed. Initialization failure attempts shutdown before publishing Failed. Shutdown hooks must be idempotent because cleanup can be retried after partial initialization. Start each plugin explicitly, then stop it or call `shutdown().await` before destroying its host. Serialize lifecycle control: stopping a plugin while initialization is still in progress returns Unavailable. A runtime-wide shutdown prevents a subsequently completed initializer from becoming Ready and asks it to release resources. Concurrent stop callers do not wait for another caller's in-progress cleanup. A cancelled initialization/shutdown future cannot guarantee asynchronous cleanup; resource-owning plugin implementations must also use RAII/Drop for partial initialization. Keep the lifecycle driver alive through awaited shutdown.

Holding a pending call retains its Rust runtime owner. Dropping an arbitrary runtime clone is not a substitute for explicit shutdown. Destroy host bridge objects only after stopping registered proxies; then detach the bridge and dispose host callbacks.

## Desktop adapter and examples

```sh
cargo run -p native-plugins --locked
```

This demonstration creates a new temporary directory, grants only filesystem access to that root, writes/reads a small file, receives a typed change event, shuts down, and removes its own directory. It does not send notifications or use a camera.

- `FilesystemPlugin::new(root)` pins an existing trusted root. Unix implementations walk descriptor-relative paths with `O_NOFOLLOW`, reject parent/absolute traversal and non-regular files, validate before truncating, and bound reads/writes to 256 KiB. Parent directories must already exist. This protects against symlink replacement; it is not an inode-isolation mechanism against malicious hard links inside a root controlled by an attacker. The root must be app-owned/trusted. Windows rooted file operations are explicitly unsupported by this adapter.
- `NotificationsPlugin` uses `/usr/bin/osascript` on macOS and `/usr/bin/notify-send` on Linux, passing title/body as separate arguments rather than interpolating commands. It advertises availability only when the service executable exists. Delivery, app notification settings, and platform entitlements still belong to the OS; a successful request does not prove the user saw a notification. Windows notifications are currently unavailable.
- `CameraPlugin::new(backend)` installs an explicit `CameraBackend`. `FfmpegCamera::new(executable, device)` supports an already-installed FFmpeg on macOS AVFoundation and Linux video4linux2. The trusted host fixes the executable and device. Capture checks permission, bounds dimensions/output, kills/reaps a cancelled or timed-out child, and has a ten-second capture deadline. Registration never installs software or activates a camera. Native application bundles need their normal camera entitlements/usage descriptions. A fake backend is used for tests, not production success responses.

Calls can be awaited from Omen's existing async action/lifecycle blocks. `omen::plugins` reexports the registry API. For example, supply `Observable<PluginRuntime>` through `.environment(...)`, resolve `@Environment let native: PluginRuntime;`, and await `native.call(ReadText { ... })` in `.task async { ... }` or a Button async action. Keep startup/shutdown under application ownership; do not register a new runtime every render.

## Versioned bridge protocol

All hosts use UTF-8 JSON with `api_version: 1` and decimal **string** IDs so JavaScript cannot round u64 correlations. Frames are tagged `call`, `cancel`, `response`, `error`, or `event`.

```json
{"type":"call","api_version":1,"id":"42","plugin":"omen.filesystem","operation":"read_text","payload":{"path":"hello.txt"}}
```

```json
{"type":"response","api_version":1,"id":"42","payload":"Hello"}
```

Errors use `{ "code": "permission_denied", "message": "..." }`; other codes are not_found, invalid_request, unsupported, incompatible_version, unavailable, cancelled, native, and lagged. Events include plugin, topic, and payload. Messages are size checked before parsing; unknown/stale response IDs and incompatible majors are rejected. IDs cannot be reused while pending. Native transports must copy outgoing borrowed buffers synchronously and return without waiting for a response. Immediate/reentrant responses are supported.

`NativeBridge::plugin(descriptor)` creates a proxy registered with `PluginRuntime` in the ordinary way. One bridge should belong to one runtime; do not attach the same plugin ID on a bridge to several runtime owners. NativeBridge holds startup contexts for authorized unsolicited events and releases them during shutdown/detachment.

`API_VERSION` is the wire/API major, not the crate version. The descriptor's implementation version is informational; it does not automatically negotiate schemas. App and host must ship matching request/output/event schemas. Breaking schema or C ABI changes require a new major/versioned entry point; do not depend on Rust trait-object layout across a dynamic-library boundary. Wire structs reject unknown top-level fields, so adding envelope fields also requires a coordinated update. There is no automatic schema migration or unsigned dynamic library loader.

## Android Kotlin/JNI

`platforms/android` is an Android library using AGP 9.1 built-in Kotlin, SDK 36, min SDK 26, and NDK 28.2.13676358. Its default ABI is arm64-v8a; build and package additional Rust targets before adding other ABI filters.

1. Build `omen-plugin-ffi` for `aarch64-linux-android`, setting `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` to your NDK's `aarch64-linux-android26-clang`.
2. Copy `target/aarch64-linux-android/release/libomen_plugin_ffi.so` into `platforms/android/src/main/jniLibs/arm64-v8a/`.
3. With SDK/NDK installed, run `gradle --no-daemon -p platforms/android assembleDebug` using Gradle 9.3.1/JDK 21.
4. Construct `OmenBridge(activity, allowlist)` before the Activity reaches STARTED. Pass its numeric `handle` into the application's Rust bootstrap and retrieve `omen_plugin_ffi::bridge(handle)` to register the reviewed standard descriptors/proxies.
5. Request native permissions from explicit UI actions and synchronize results to the Rust permission broker. Stop Rust plugins, then close the Kotlin host from the Activity lifecycle.

The JNI shim copies byte arrays, attaches/detaches Rust callback threads, clears native-call Java exceptions into transport failures, and releases global references on close. CMake links the Rust shared library into `omen_android`. Consumer rules retain the JNI entry points.

Kotlin operations run through supervised coroutines, marshal UI work to Main, and move file I/O to Dispatchers.IO. Files are limited to single filenames in `filesDir` with final-component no-follow opens. Notifications use a notification channel and check current authorization. Camera capture uses `TakePicturePreview`; its output is a platform-provided thumbnail scaled within the requested bounds, not a full-resolution capture guarantee. OS Activity results cannot be forcibly recalled; cancelled results are ignored. The manifest declares CAMERA/POST_NOTIFICATIONS; the application must review its merged permissions.

API references: [runtime permission requests](https://developer.android.com/training/permissions/requesting), [TakePicturePreview](https://developer.android.com/reference/androidx/activity/result/contract/ActivityResultContracts.TakePicturePreview), [AGP built-in Kotlin](https://developer.android.com/build/migrate-to-built-in-kotlin).

## iOS Swift/FFI

`platforms/ios` provides a Swift package and C module header. Build the Rust static library for each required iOS/device/simulator target, supply its library search path to the app, and link `omen_plugin_ffi` exactly once. Package/XCFramework distribution and code signing remain application build steps.

Construct `OmenNativeBridge(presenter:allowlist:root:)` on MainActor, pass `handle` to the Rust bootstrap, and register the reviewed bridge proxies. The C callback copies bytes before returning and dispatches onto MainActor through a weak host registry. No Swift/Rust heap allocation is freed by the other language. C pointer/length contracts are documented in the header and Rust safety docs; callers must pass valid readable buffers and keep callback code alive through destruction/in-flight callbacks.

Swift uses rooted POSIX no-follow file access off the main actor, UserNotifications, and UIImagePickerController camera UI with AVFoundation authorization. Add NSCameraUsageDescription and applicable app entitlements. Permission requests belong to user actions. Explicitly stop Rust plugins, then call `close()` when the scene ends; pending tasks and camera presentation are cancelled. OS work already scheduled cannot always be undone. File operations are bounded but a file task already running may finish after cancellation.

This source is type-checked against the installed iOS simulator SDK. Device/simulator execution, native prompts, entitlements, and linked application packaging require an actual host app; no hardware capture was performed during repository validation. See [Swift C interoperability](https://developer.apple.com/documentation/swift/c-interoperability).

## Web JavaScript/WASM

Build with `cargo build -p omen-plugin-web --target wasm32-unknown-unknown --release`, then generate the normal wasm-bindgen/wasm-pack bundler output. `WebBridge` imports `platforms/web/host.mjs`; include that module in your bundle. Instantiate `WebBridge(context)`, install `createNativeHost({ context, allowlist, receive: bytes => bridge.receive(bytes) })`, then register bridge proxies from application Rust through `native_bridge()`. The host registry uses an origin-local Symbol key so copied wasm-bindgen snippets and the application import share the same context map. Use unique u32 contexts, and close both endpoints on teardown.

The adapter uses origin-private storage (OPFS), browser notifications, and getUserMedia/video/canvas for still JPEG capture. It never exposes arbitrary host filesystem paths. Secure-context/API availability and browser grants remain authoritative; call `host.requestPermission(...)` from a user gesture and mirror its result into the Rust host broker. Camera tracks stop in finally blocks, including cancelled capture; a pending permission prompt itself cannot always be cancelled. Browser support differs, and notifications in environments requiring a service worker are not supplied by this basic constructor-based adapter. The WASM binding is intended for the browser main thread, not arbitrary web workers.

References: [getUserMedia security constraints](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia), [secure-context APIs](https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Secure_Contexts/features_restricted_to_secure_contexts).

## Validation

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check -p omen-plugin-web --target wasm32-unknown-unknown --locked
node --test platforms/web/host.test.mjs
```

Additional platform checks compile Kotlin, syntax-check/link JNI against the Android NDK, build the ARM64 Rust bridge, package an Android AAR, and type-check Swift against the iOS simulator SDK. Rust tests cover denial, revocation, lifecycle, cancellation, event overflow, immediate bridge responses, malformed frames, detachment, root traversal/symlinks, and hardware-free camera contracts. JavaScript tests use in-memory origin storage and fake notification services. Hardware behavior and end-user permission UI must be verified in the embedding applications before release.
