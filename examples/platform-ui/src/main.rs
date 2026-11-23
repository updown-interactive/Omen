#[cfg(not(target_arch = "wasm32"))]
use omen::plugins::PermissionStore;
use omen::plugins::standard::{ReadText, WriteText};
use omen::plugins::{ErrorCode, PermissionStatus, PluginError, PluginRuntime};
use omen::{
    AnyView, AppLifecycle, SwipeDirection, ThemeMode, Value, ViewExt, Viewport, main, view,
};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use std::time::Duration;
view! {
    Home {
        @State let count=0
        @State let name=String::new()
        @State let enabled=true
        @State let modal=false
        @State let dark=true
        @State let status=String::from("Ready")
        @Environment viewport:Viewport;
        @Environment lifecycle:AppLifecycle;
        @Environment native:PluginRuntime;
        @State let native_ready=false
        Layout {
            Text("Omen across platforms").font(.largeTitle).accessibilityRole(.heading)
            Text("Count: {count}").font(.title)
            Button("Increment"){count+=1;}.padding(12)
            TextField("Your name").value($name)
            Text("Hello, {name}")
            Toggle("Enabled").checked($enabled)
            Text(if enabled {"Enabled"}else{"Disabled"})
            Button("Toggle theme"){dark=!dark;}.padding(12)
            Button("Open dialog"){modal=true;}.padding(12)
            Button("Async action") async {status="Working".into();omen::sleep(Duration::from_millis(100)).await;status="Complete".into();}.padding(12)
            if cfg!(target_arch="wasm32") {
                Button("Native file round trip") async {
                    if !native_ready {native.start("omen.filesystem").await?;native_ready=true;}
                    for permission in ["fs.read","fs.write"] {
                        if native.request_permission(permission).await?!=PermissionStatus::Granted{return Err(PluginError::new(ErrorCode::PermissionDenied,"Native permission denied"));}
                    }
                    native.call(WriteText{path:"omen-demo.txt".into(),contents:"Hello from Rust".into()}).await?;
                    let contents=native.call(ReadText{path:"omen-demo.txt".into()}).await?;
                    status=format!("Native file: {contents}");
                    Ok::<(),PluginError>(())
                }.onError(|error|{status=error;}).padding(12)
            }
            Text(status)
            Text("Hold or swipe left").padding(24).background(.surface)
                .onLongPress {count+=10;}
                .onSwipe(SwipeDirection::Left,||{count+=100;})
            Text("Width: {viewport.width:.0} · {lifecycle:?}").font(.caption)
        }.spacing(12).padding(24)
        .dialog($modal,||view!{Layout{Text("A cross-platform dialog").font(.title) Text("Escape or tap outside to close")}.spacing(12)})
        .theme(if dark{ThemeMode::Dark}else{ThemeMode::Light}).safeArea(.all)
    }
}
#[cfg(target_arch = "wasm32")]
fn plugins() -> Result<PluginRuntime, PluginError> {
    let runtime = PluginRuntime::new(["fs.read", "fs.write"], omen::web::permission_broker());
    runtime.register(
        omen::web::native_bridge().plugin(omen::plugins::standard::filesystem_descriptor()),
    )?;
    Ok(runtime)
}
#[cfg(not(target_arch = "wasm32"))]
fn plugins() -> Result<PluginRuntime, PluginError> {
    Ok(PluginRuntime::new(
        Vec::<String>::new(),
        Arc::new(PermissionStore::default()),
    ))
}
fn root() -> AnyView {
    match plugins() {
        Ok(runtime) => view! {Home()}.environment(Value::new(runtime)),
        Err(error) => view! {Text(error.to_string())},
    }
}
main! {root()}
