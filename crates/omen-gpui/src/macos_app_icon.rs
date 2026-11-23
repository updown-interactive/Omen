use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::{NSBundle, NSString};

pub(super) fn apply_bundle_icon() {
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let bundle = NSBundle::mainBundle();
    let key = NSString::from_str("CFBundleIconFile");
    let Some(name) = bundle
        .objectForInfoDictionaryKey(&key)
        .and_then(|value| value.downcast::<NSString>().ok())
    else {
        return;
    };
    let Some(url) = bundle
        .URLForResource_withExtension(Some(&name), None)
        .or_else(|| {
            bundle.URLForResource_withExtension(Some(&name), Some(&NSString::from_str("icns")))
        })
    else {
        return;
    };
    let Some(icon) = NSImage::initWithContentsOfURL(main_thread.alloc(), &url) else {
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    // CLI runs spawn the bundle executable directly to retain its output and
    // process handle. Apply the icon after AppKit has registered the Dock tile.
    unsafe { app.setApplicationIconImage(Some(&icon)) };
    crate::native_trace("applied macOS Dock icon from the app bundle");
}
