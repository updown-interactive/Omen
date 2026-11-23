use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "omen-cli-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn omen() -> Command {
    Command::new(env!("CARGO_BIN_EXE_omen"))
}
fn sdk() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
#[test]
fn creates_an_isolated_project_with_one_shared_entry_file_without_overwriting() {
    let sandbox = Sandbox::new();
    success(
        omen()
            .current_dir(&sandbox.0)
            .args(["new", "my-store", "--sdk"])
            .arg(sdk())
            .output()
            .unwrap(),
    );
    let root = sandbox.0.join("my-store");
    let cargo: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    assert_eq!(cargo["lib"]["name"].as_str(), Some("omen_app"));
    assert!(cargo.get("workspace").is_some());
    assert_eq!(cargo["lib"]["path"].as_str(), Some("src/main.rs"));
    assert!(cargo["dependencies"].get("omen").is_some());
    assert!(cargo["dependencies"].get("omen-ui").is_none());
    assert!(!root.join("src/android.rs").exists());
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Omen.toml")).unwrap()).unwrap();
    assert_eq!(config["app"]["id"].as_str(), Some("com.example.mystore"));
    assert_eq!(config["android"]["renderer"].as_str(), Some("native"));
    let source = fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert!(source.contains("use views::Home;"));
    assert!(source.contains("main! { Home() }"));
    let home = fs::read_to_string(root.join("src/views/home_view.rs")).unwrap();
    assert!(home.contains("@Design\n    pub Home {"));
    fs::write(root.join("keep"), "user work").unwrap();
    assert!(
        !omen()
            .current_dir(&sandbox.0)
            .args(["new", "my-store"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(root.join("keep")).unwrap(), "user work");
}
#[test]
fn starter_avoids_using_an_sdk_package_name() {
    use omen_cli::project::template;

    let sandbox = Sandbox::new();
    let sdk = sdk();
    template::create_in("omen-web", &sandbox.0, Some(&sdk)).unwrap();
    let root = sandbox.0.join("omen-web");
    let cargo: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    assert_eq!(cargo["package"]["name"].as_str(), Some("omen-web-app"));
    let manifest = omen_cli::project::manifest::Manifest::read(&root.join("Omen.toml")).unwrap();
    assert_eq!(manifest.app.name, "omen-web");

    let mut manifest = template::default_manifest("another-app", Some(sdk));
    manifest.app.package = Some("omen-web".into());
    let error = template::create_configured("another-app", &sandbox.0, None, manifest).unwrap_err();
    assert!(error.to_string().contains("used by the Omen SDK"));
    assert!(!sandbox.0.join("another-app").exists());
}

#[test]
fn rejects_path_traversal_and_invalid_renderer_configuration() {
    let sandbox = Sandbox::new();
    assert!(
        !omen()
            .current_dir(&sandbox.0)
            .args(["new", "../escape"])
            .output()
            .unwrap()
            .status
            .success()
    );
    success(
        omen()
            .current_dir(&sandbox.0)
            .args(["new", "app"])
            .output()
            .unwrap(),
    );
    let path = sandbox.0.join("app/Omen.toml");
    fs::write(
        &path,
        fs::read_to_string(&path)
            .unwrap()
            .replace("native", "webview"),
    )
    .unwrap();
    let result = omen()
        .arg("--project")
        .arg(sandbox.0.join("app"))
        .args(["build", "-p", "web"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Supported renderers"));
}
#[test]
fn requires_platform_in_noninteractive_runs_instead_of_hanging() {
    let sandbox = Sandbox::new();
    success(
        omen()
            .current_dir(&sandbox.0)
            .args(["new", "app"])
            .output()
            .unwrap(),
    );
    let result = omen()
        .current_dir(sandbox.0.join("app"))
        .arg("run")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--platform"));
}
#[cfg(unix)]
#[test]
fn build_uses_external_project_config_and_packager_artifact_manifest() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let root = &sandbox.0;
    let fake_sdk = root.join("sdk");
    fs::create_dir_all(fake_sdk.join("scripts")).unwrap();
    fs::create_dir_all(fake_sdk.join("crates/omen")).unwrap();
    fs::write(fake_sdk.join("crates/omen/Cargo.toml"), "").unwrap();
    fs::write(fake_sdk.join("scripts/package.py"),r#"import argparse,json,pathlib
p=argparse.ArgumentParser();p.add_argument('platform');p.add_argument('--project-root');p.add_argument('--config');p.add_argument('--target-dir');p.add_argument('--out');p.add_argument('--icons');p.add_argument('--release',action='store_true');a=p.parse_args()
c=json.loads(pathlib.Path(a.config).read_text());out=pathlib.Path(a.out);out.mkdir(parents=True)
assert (pathlib.Path(a.icons)/'favicon.png').read_bytes().startswith(b'\x89PNG')
(out/'config-used.json').write_text(json.dumps({'project':a.project_root,'config':c,'release':a.release}))
(out/'omen-artifact.json').write_text(json.dumps({'platform':a.platform,'path':str(out),'bundle_id':c['identifier']}))
"#).unwrap();
    success(
        omen()
            .current_dir(root)
            .args(["new", "shop", "--sdk"])
            .arg(&fake_sdk)
            .output()
            .unwrap(),
    );
    image::RgbaImage::from_pixel(32, 32, image::Rgba([30, 60, 90, 255]))
        .save(root.join("shop/assets/icon.png"))
        .unwrap();
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let sysroot = root.join("rust");
    fs::create_dir_all(sysroot.join("lib/rustlib/wasm32-unknown-unknown/lib")).unwrap();
    fs::write(
        sysroot.join("lib/rustlib/wasm32-unknown-unknown/lib/libstd-test.rlib"),
        "",
    )
    .unwrap();
    for (name, body) in [
        ("cargo", "#!/bin/sh\nexit 0\n".to_owned()),
        (
            "rustc",
            format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", sysroot.display()),
        ),
    ] {
        let file = bin.join(name);
        fs::write(&file, body).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    std::os::unix::fs::symlink(
        String::from_utf8(python.stdout).unwrap().trim(),
        bin.join("python3"),
    )
    .unwrap();
    let custom = root.join("custom-target");
    success(
        omen()
            .current_dir(root.join("shop"))
            .env("PATH", &bin)
            .env("CARGO_TARGET_DIR", &custom)
            .args(["build", "-p", "web", "--release"])
            .output()
            .unwrap(),
    );
    let used: serde_json::Value = serde_json::from_slice(
        &fs::read(custom.join("omen/packages/web/config-used.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(used["config"]["identifier"], "com.example.shop");
    assert_eq!(used["release"], true);
    assert_eq!(
        used["project"],
        root.join("shop")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert!(!fake_sdk.join("target").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn ios_launch_failure_is_reported_without_claiming_ready() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let root = &sandbox.0;
    let sdk = root.join("sdk");
    fs::create_dir_all(sdk.join("scripts")).unwrap();
    fs::create_dir_all(sdk.join("crates/omen")).unwrap();
    fs::write(sdk.join("crates/omen/Cargo.toml"), "").unwrap();
    fs::write(sdk.join("scripts/package.py"),r#"import argparse,json,pathlib
p=argparse.ArgumentParser();p.add_argument('platform');p.add_argument('--project-root');p.add_argument('--config');p.add_argument('--target-dir');p.add_argument('--out');a=p.parse_args();out=pathlib.Path(a.out);out.mkdir(parents=True);bundle=out/'Test.app';bundle.mkdir();c=json.loads(pathlib.Path(a.config).read_text());(out/'omen-artifact.json').write_text(json.dumps({'platform':'ios','path':str(bundle),'bundle_id':c['identifier']}))
"#).unwrap();
    success(
        omen()
            .current_dir(root)
            .args(["new", "phone", "--sdk"])
            .arg(&sdk)
            .output()
            .unwrap(),
    );
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let sysroot = root.join("rust");
    fs::create_dir_all(sysroot.join("lib/rustlib/aarch64-apple-ios-sim/lib")).unwrap();
    fs::write(
        sysroot.join("lib/rustlib/aarch64-apple-ios-sim/lib/libstd-test.rlib"),
        "",
    )
    .unwrap();
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in -vV) echo 'host: aarch64-apple-darwin';; *) echo '{}';; esac\n",
        sysroot.display()
    );
    for (name,body) in [("cargo","#!/bin/sh\nexit 0\n".to_owned()),("rustc",script),("xcrun",r#"#!/bin/sh
case "$1:$2:$3" in
--sdk:iphonesimulator:--show-sdk-path) echo /fake-sdk;;
simctl:list:devices) echo '{"devices":{"iOS-26":[{"udid":"test-phone","name":"iPhone 17 Pro","state":"Booted","isAvailable":true}]}}';;
simctl:launch:*) echo 'specific native launch failure' >&2; exit 42;;
*) exit 0;;
esac
"#.to_owned())]{let p=bin.join(name);fs::write(&p,body).unwrap();fs::set_permissions(p,fs::Permissions::from_mode(0o755)).unwrap();}
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    std::os::unix::fs::symlink(
        String::from_utf8(python.stdout).unwrap().trim(),
        bin.join("python3"),
    )
    .unwrap();
    let output = omen()
        .current_dir(root.join("phone"))
        .env("PATH", bin)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(["run", "-p", "ios", "--device", "test-phone"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("launch is unverified"));
    assert!(error.contains("specific native launch failure"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Launched process"));
}

#[cfg(unix)]
#[test]
fn rustup_toolchain_overrides_shadowing_path_for_usb_discovery_and_packaging() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let root = sandbox.0.canonicalize().unwrap();
    let system = root.join("system");
    let managed = root.join("managed/bin");
    let sdk = root.join("sdk");
    let android = root.join("android");
    let java = root.join("java");
    let target = root.join("artifacts");
    for dir in [
        &system,
        &managed,
        &sdk.join("scripts"),
        &sdk.join("crates/omen"),
        &java.join("bin"),
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    let script = |path: PathBuf, body: String| {
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    let brewed = root.join("brew-sysroot");
    let active = root.join("active-sysroot");
    for sysroot in [&brewed, &active] {
        let lib = sysroot.join("lib/rustlib/aarch64-apple-darwin/lib");
        fs::create_dir_all(&lib).unwrap();
        fs::write(lib.join("libstd-host.rlib"), "").unwrap();
    }
    let native_lib = active.join("lib/rustlib/aarch64-linux-android/lib");
    fs::create_dir_all(&native_lib).unwrap();
    fs::write(native_lib.join("libstd-android.rlib"), "").unwrap();
    script(
        system.join("rustc"),
        format!("#!/bin/sh\necho '{}'\n", brewed.display()),
    );
    script(system.join("cargo"), "#!/bin/sh\nexit 99\n".into());
    script(
        managed.join("rustc"),
        format!(
            "#!/bin/sh\ncase \"$1\" in -vV) echo 'host: aarch64-apple-darwin';; *) echo '{}';; esac\n",
            active.display()
        ),
    );
    script(managed.join("cargo"), "#!/bin/sh\nexit 0\n".into());
    script(
        system.join("rustup"),
        format!("#!/bin/sh\necho '{}'\n", managed.join("rustc").display()),
    );
    script(system.join("adb"),"#!/bin/sh\nprintf 'List of devices attached\\nusb-test device usb:1-2 model:V2413 transport_id:1\\n'\n".into());
    script(system.join("gradle"), "#!/bin/sh\nexit 0\n".into());
    script(
        java.join("bin/java"),
        "#!/bin/sh\necho 'openjdk version \"21.0.0\"' >&2\n".into(),
    );
    let host = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let compiler = android
        .join("ndk/28.2.13676358/toolchains/llvm/prebuilt")
        .join(host)
        .join("bin/aarch64-linux-android26-clang");
    fs::create_dir_all(compiler.parent().unwrap()).unwrap();
    fs::write(compiler, "").unwrap();
    fs::write(sdk.join("crates/omen/Cargo.toml"), "").unwrap();
    fs::write(sdk.join("scripts/package.py"),r#"import argparse,json,pathlib,shutil,subprocess
p=argparse.ArgumentParser();p.add_argument('platform');p.add_argument('--project-root');p.add_argument('--config');p.add_argument('--target-dir');p.add_argument('--out');a=p.parse_args()
out=pathlib.Path(a.out);out.mkdir(parents=True);apk=out/'native.apk';apk.write_bytes(b'test')
(out/'compiler-used.json').write_text(json.dumps({'cargo':shutil.which('cargo'),'sysroot':subprocess.check_output(['rustc','--print','sysroot'],text=True).strip()}))
(out/'omen-artifact.json').write_text(json.dumps({'platform':'android','path':str(apk),'bundle_id':'com.example.phone'}))
"#).unwrap();
    success(
        omen()
            .current_dir(&root)
            .args(["new", "phone", "--sdk"])
            .arg(&sdk)
            .output()
            .unwrap(),
    );
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    std::os::unix::fs::symlink(
        String::from_utf8(python.stdout).unwrap().trim(),
        system.join("python3"),
    )
    .unwrap();
    let command = || {
        let mut c = omen();
        c.current_dir(&root)
            .arg("--project")
            .arg(root.join("phone"))
            .env("PATH", &system)
            .env_remove("RUSTC")
            .env("ANDROID_HOME", &android)
            .env_remove("ANDROID_NDK_HOME")
            .env("JAVA_HOME", &java)
            .env("GRADLE", system.join("gradle"))
            .env("CARGO_TARGET_DIR", &target);
        c
    };
    let doctor: serde_json::Value = serde_json::from_str(&success(
        command().args(["doctor", "--json"]).output().unwrap(),
    ))
    .unwrap();
    let status = doctor["platforms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["platform"] == "android")
        .unwrap();
    assert_eq!(status["can_build"], true);
    assert_eq!(status["connected_devices"][0], "V2413");
    assert_eq!(
        doctor["tools"]["rustc"],
        managed.join("rustc").to_string_lossy().as_ref()
    );
    success(command().args(["build", "-p", "android"]).output().unwrap());
    let used: serde_json::Value = serde_json::from_slice(
        &fs::read(target.join("omen/packages/android/compiler-used.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        used["cargo"],
        managed.join("cargo").to_string_lossy().as_ref()
    );
    assert_eq!(used["sysroot"], active.to_string_lossy().as_ref());
    // No switching to some other installed toolchain just to make a target appear.
    fs::remove_file(native_lib.join("libstd-android.rlib")).unwrap();
    let doctor: serde_json::Value = serde_json::from_str(&success(
        command().args(["doctor", "--json"]).output().unwrap(),
    ))
    .unwrap();
    let status = doctor["platforms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["platform"] == "android")
        .unwrap();
    assert_eq!(status["can_build"], false);
    assert_eq!(status["connected_devices"][0], "V2413");
}

#[test]
fn disabled_targets_are_rejected_before_building_or_selecting_devices() {
    use omen_cli::project::{manifest::TargetPlatform, template};
    let sandbox = Sandbox::new();
    let mut manifest = template::default_manifest("web-app", Some(sdk()));
    manifest.targets.platforms = vec![TargetPlatform::Web];
    template::create_configured("web-app", &sandbox.0, Some(&sdk()), manifest).unwrap();
    for command in ["build", "run"] {
        let output = omen()
            .current_dir(sandbox.0.join("web-app"))
            .args([command, "--platform", "android"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not enabled in Omen.toml"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn generate_icon_uses_manifest_source_and_only_generates_enabled_platforms() {
    use omen_cli::project::{manifest::TargetPlatform, template};
    let sandbox = Sandbox::new();
    let mut manifest = template::default_manifest("icon-test", None);
    manifest.app.icon = Some("logo.png".into());
    manifest.targets.platforms = vec![TargetPlatform::Android, TargetPlatform::Web];
    fs::write(
        sandbox.0.join("Omen.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let source = sandbox.0.join("logo.png");
    image::RgbaImage::from_pixel(48, 48, image::Rgba([10, 20, 30, 40]))
        .save(&source)
        .unwrap();

    // A logo and manifest are sufficient, even with no Cargo.toml.
    let icons = sandbox.0.join(".omen/icons");
    success(
        omen()
            .current_dir(&sandbox.0)
            .args(["generate-icon", "--platform", "android"])
            .output()
            .unwrap(),
    );
    assert!(!icons.join("web").exists());
    success(
        omen()
            .current_dir(&sandbox.0)
            .arg("generate-icon")
            .output()
            .unwrap(),
    );
    assert_eq!(
        image::image_dimensions(icons.join("android/mipmap-xxxhdpi/omen_app_icon.png")).unwrap(),
        (192, 192)
    );
    assert_eq!(
        image::image_dimensions(icons.join("web/favicon.png")).unwrap(),
        (32, 32)
    );
    assert!(!icons.join("desktop").exists());
    let disabled = omen()
        .current_dir(&sandbox.0)
        .args(["generate-icon", "--platform", "desktop"])
        .output()
        .unwrap();
    assert!(!disabled.status.success());
    assert!(String::from_utf8_lossy(&disabled.stderr).contains("not enabled in Omen.toml"));
    image::RgbaImage::new(32, 16).save(source).unwrap();
    let invalid = omen()
        .current_dir(&sandbox.0)
        .arg("generate-icon")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("must be square"));
}
