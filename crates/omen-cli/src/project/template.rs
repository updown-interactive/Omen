use super::manifest::*;
use anyhow::{Context, Result};
use std::{collections::HashSet, fs, io, path::Path};

pub fn create(name: &str, sdk: Option<&Path>) -> Result<()> {
    create_in(name, &std::env::current_dir()?, sdk)
}

/// Generate a project in an explicit parent without changing process working directory.
pub fn create_in(name: &str, parent: &Path, sdk: Option<&Path>) -> Result<()> {
    let manifest = default_manifest(name, Some(sdk_path(sdk)?));
    create_configured(name, parent, sdk, manifest)
}

/// Parse and validate the complete project configuration before creating any files.
pub fn create_from_toml(name: &str, parent: &Path, sdk: Option<&Path>, source: &str) -> Result<()> {
    let manifest: Manifest = toml::from_str(source).context("Invalid Omen.toml")?;
    create_configured(name, parent, sdk, manifest)
}

pub fn create_configured(
    name: &str,
    parent: &Path,
    fallback_sdk: Option<&Path>,
    manifest: Manifest,
) -> Result<()> {
    create_configured_with(name, parent, fallback_sdk, manifest, |_, source| {
        source.to_owned()
    })
}

/// Generate a configured project and decorate starter files inside the creation transaction.
pub fn create_configured_with(
    name: &str,
    parent: &Path,
    fallback_sdk: Option<&Path>,
    mut manifest: Manifest,
    decorate: impl Fn(&Path, &str) -> String,
) -> Result<()> {
    validate_name(name)?;
    manifest.validate()?;
    let configured = manifest.toolchain.sdk.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            parent.join(name).join(path)
        }
    });
    let sdk = sdk_path(configured.as_deref().or(fallback_sdk))?;
    let reserved = sdk_packages(&sdk)?;
    let package = match &manifest.app.package {
        Some(package) => package.clone(),
        None if reserved.contains(name) => format!("{name}-app"),
        None => name.to_owned(),
    };
    validate_name(&package)?;
    if reserved.contains(&package) {
        anyhow::bail!(
            "Cargo package '{package}' is used by the Omen SDK. Choose a different app.package in Omen.toml."
        );
    }
    if manifest
        .app
        .native_package
        .as_ref()
        .is_some_and(|native| native != &package)
    {
        anyhow::bail!(
            "A generated starter has one package; app.native_package must match {package}"
        );
    }
    manifest.toolchain.sdk = Some(sdk.clone());
    let version = &manifest.app.version;
    let default_backend = if manifest
        .targets
        .platforms
        .iter()
        .any(|target| *target != TargetPlatform::Web)
    {
        "native"
    } else {
        "web"
    };
    let root = parent.join(name);
    std::fs::create_dir(&root).with_context(|| {
        format!(
            "Cannot create {}; existing directories are never overwritten",
            root.display()
        )
    })?;
    let write = |path: &str, source: &str| {
        let path = Path::new(path);
        std::fs::write(root.join(path), decorate(path, source))
    };
    let result = (|| -> Result<()> {
        std::fs::create_dir_all(root.join("src/views"))?;
        std::fs::create_dir(root.join("assets"))?;
        let sdk_dependency = serde_json::to_string(&sdk.join("crates/omen").to_string_lossy())?;
        write(
            "Cargo.toml",
            &format!(
                r#"[package]
name = "{package}"
version = "{version}"
edition = "2024"

[workspace]

[dependencies]
omen = {{ path = {sdk_dependency}, default-features = false }}

[features]
default = ["{default_backend}"]
native = ["omen/native"]
web = ["omen/web"]

[lib]
name = "omen_app"
path = "src/main.rs"
crate-type = ["cdylib"]

[profile.mobile]
inherits = "dev"
opt-level = 1
debug = 0
incremental = false
"#
            ),
        )?;
        write("Omen.toml", &toml::to_string_pretty(&manifest)?)?;
        write(
            "src/main.rs",
            r#"use omen::main;

mod views;
use views::Home;

main! { Home() }
"#,
        )?;
        write(
            "src/views/home_view.rs",
            r#"use omen::prelude::*;

view! {
    @Design
    pub Home {
        Layout {
            Text("Hello, Omen!")
                .font(.largeTitle)
                .weight(.bold)

            Button("Get Started") {
                println!("Welcome to Omen!");
            }
            .padding(16)
            .background(.blue)
            .color(.white)
        }
        .spacing(.lg)
        .padding(.xl)
        .safeArea(.all)
    }
}
"#,
        )?;
        write(
            "src/views/mod.rs",
            "mod home_view;\npub use home_view::Home;\n",
        )?;
        std::fs::write(root.join("assets/.gitkeep"), "")?;
        write(".gitignore", "/target/\n/.omen/\n.DS_Store\n")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result?;
    println!("✓ Created Omen project: {name}\n\nNext steps:\n  cd {name}\n  omen run");
    Ok(())
}

fn sdk_packages(sdk: &Path) -> Result<HashSet<String>> {
    let mut names = HashSet::new();
    for entry in fs::read_dir(sdk.join("crates"))? {
        let path = entry?.path().join("Cargo.toml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                continue;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot read {}", path.display()));
            }
        };
        let manifest: toml::Value = toml::from_str(&source)
            .with_context(|| format!("Invalid SDK manifest {}", path.display()))?;
        if let Some(name) = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
        {
            names.insert(name.to_owned());
        }
    }
    Ok(names)
}

/// Pure defaults shared by the CLI and Studio's project configuration UI.
pub fn default_manifest(name: &str, sdk: Option<std::path::PathBuf>) -> Manifest {
    let title = name
        .split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    Manifest {
        app: App {
            name: name.into(),
            id: format!("com.example.{}", name.replace(['-', '_'], "")),
            version: "0.1.0".into(),
            icon: None,
            package: None,
            native_package: None,
        },
        targets: Targets::default(),
        window: Window {
            title,
            width: 1100,
            height: 720,
        },
        android: Renderer {
            renderer: "native".into(),
        },
        ios: Renderer {
            renderer: "native".into(),
        },
        web: Renderer {
            renderer: "dom".into(),
        },
        toolchain: Toolchain { sdk },
    }
}
