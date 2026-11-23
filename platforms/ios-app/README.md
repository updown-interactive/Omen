# Native iOS application

Build with `python3 scripts/package.py ios`. The Rust executable calls GPUI's
UIKit application entry point and renders with Metal. No Swift application host,
WKWebView or Xcode app project is required. GPUI's pinned iOS implementation owns
UIApplication, UIWindowScene, CAMetalLayer, text input and lifecycle callbacks.

The superseded WebView prototype is archived under `platforms/legacy-webview`.
Use `scripts/ios-smoke.py` to install and capture the native simulator build.
