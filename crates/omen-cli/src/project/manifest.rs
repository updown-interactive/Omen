use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_package: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Window {
    pub title: String,
    pub width: u32,
    pub height: u32,
}
impl Default for Window {
    fn default() -> Self {
        Self {
            title: "Omen".into(),
            width: 1100,
            height: 720,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Renderer {
    pub renderer: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Toolchain {
    pub sdk: Option<PathBuf>,
}
/// Supported Omen build-target families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetPlatform {
    Desktop,
    Android,
    Ios,
    Web,
}
impl TargetPlatform {
    pub const ALL: [Self; 4] = [Self::Desktop, Self::Android, Self::Ios, Self::Web];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Web => "web",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Targets {
    pub platforms: Vec<TargetPlatform>,
}
impl Default for Targets {
    fn default() -> Self {
        Self {
            platforms: TargetPlatform::ALL.to_vec(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub app: App,
    #[serde(default)]
    pub targets: Targets,
    #[serde(default)]
    pub window: Window,
    pub android: Renderer,
    pub ios: Renderer,
    pub web: Renderer,
    #[serde(default)]
    pub toolchain: Toolchain,
}
impl Manifest {
    pub fn supports_target(&self, platform: &str) -> bool {
        self.targets
            .platforms
            .iter()
            .any(|target| target.as_str() == platform)
    }
    pub fn require_target(&self, platform: &str) -> Result<()> {
        if !self.supports_target(platform) {
            bail!("Target '{platform}' is not enabled in Omen.toml [targets].platforms");
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read {}", path.display()))?;
        let manifest: Self =
            toml::from_str(&source).with_context(|| format!("Invalid {}", path.display()))?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .app
            .icon
            .as_ref()
            .is_some_and(|icon| icon.is_absolute() || icon.as_os_str().is_empty())
        {
            bail!("app.icon must be a nonempty PNG path relative to Omen.toml");
        }
        let unique = self
            .targets
            .platforms
            .iter()
            .collect::<std::collections::HashSet<_>>();
        if unique.is_empty() {
            bail!("Select at least one target platform");
        }
        if unique.len() != self.targets.platforms.len() {
            bail!("Target platforms must not contain duplicates");
        }
        if self.app.name.trim().is_empty() || self.app.name.chars().any(char::is_control) {
            bail!("app.name must be a nonempty single line")
        }
        let segments: Vec<_> = self.app.id.split('.').collect();
        if segments.len() < 2
            || segments.iter().any(|s| {
                s.is_empty()
                    || !s.as_bytes()[0].is_ascii_alphabetic()
                    || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
        {
            bail!("app.id must be a dotted identifier such as com.example.myapp")
        }
        if self.app.version.split('.').count() != 3
            || self
                .app
                .version
                .split('.')
                .any(|s| s.parse::<u32>().is_err())
        {
            bail!("app.version must be a numeric major.minor.patch version")
        }
        for package in [&self.app.package, &self.app.native_package]
            .into_iter()
            .flatten()
        {
            validate_name(package)?;
        }
        if !(64..=16384).contains(&self.window.width)
            || !(64..=16384).contains(&self.window.height)
            || self.window.title.chars().any(char::is_control)
        {
            bail!("window dimensions must be 64..16384 and title must be a single line")
        }
        if self.android.renderer != "native"
            || self.ios.renderer != "native"
            || self.web.renderer != "dom"
        {
            bail!("Supported renderers: android=native, ios=native, web=dom")
        }
        Ok(())
    }
}
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        || [
            "self",
            "super",
            "crate",
            "std",
            "core",
            "alloc",
            "test",
            "build",
            "deps",
            "examples",
            "incremental",
            "as",
            "break",
            "const",
            "continue",
            "else",
            "enum",
            "extern",
            "false",
            "fn",
            "for",
            "if",
            "impl",
            "in",
            "let",
            "loop",
            "match",
            "mod",
            "move",
            "mut",
            "pub",
            "ref",
            "return",
            "static",
            "struct",
            "trait",
            "true",
            "type",
            "unsafe",
            "use",
            "where",
            "while",
            "async",
            "await",
            "dyn",
            "abstract",
            "become",
            "box",
            "do",
            "final",
            "macro",
            "override",
            "priv",
            "typeof",
            "unsized",
            "virtual",
            "yield",
            "try",
            "gen",
            "con",
            "prn",
            "aux",
            "nul",
            "com1",
            "lpt1",
        ]
        .contains(&name)
    {
        bail!(
            "Use a project name starting with a lowercase letter and containing lowercase letters, digits, '-' or '_' (maximum 64 characters)"
        )
    }
    Ok(())
}
pub fn sdk_path(explicit: Option<&Path>) -> Result<PathBuf> {
    let candidate = explicit
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OMEN_SDK").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let root = candidate
        .canonicalize()
        .with_context(|| format!("Omen SDK not found at {}", candidate.display()))?;
    if !root.join("scripts/package.py").is_file() || !root.join("crates/omen/Cargo.toml").is_file()
    {
        bail!(
            "{} is not an Omen SDK checkout; set OMEN_SDK",
            root.display()
        )
    }
    Ok(root)
}
pub struct Project {
    pub root: PathBuf,
    pub sdk: PathBuf,
    pub manifest: Manifest,
    pub package: String,
    pub target: PathBuf,
}
impl Project {
    pub fn load(start: &Path) -> Result<Self> {
        let start = start
            .canonicalize()
            .context("Project directory does not exist")?;
        let root = start
            .ancestors()
            .find(|p| p.join("Omen.toml").is_file() || p.join("omen.app.json").is_file())
            .context("No Omen.toml found; run omen new <name> first")?
            .to_path_buf();
        let cargo: toml::Value =
            toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml"))?)?;
        let mut manifest: Manifest = if root.join("Omen.toml").exists() {
            Manifest::read(&root.join("Omen.toml"))?
        } else {
            let value: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(root.join("omen.app.json"))?)?;
            Manifest {
                targets: Targets::default(),
                app: App {
                    name: value["name"].as_str().context("Missing name")?.into(),
                    id: value["identifier"]
                        .as_str()
                        .context("Missing identifier")?
                        .into(),
                    version: value["version"].as_str().context("Missing version")?.into(),
                    icon: None,
                    package: value["package"].as_str().map(str::to_owned),
                    native_package: value["native_package"].as_str().map(str::to_owned),
                },
                window: Window::default(),
                android: Renderer {
                    renderer: "native".into(),
                },
                ios: Renderer {
                    renderer: "native".into(),
                },
                web: Renderer {
                    renderer: "dom".into(),
                },
                toolchain: Toolchain {
                    sdk: Some(root.clone()),
                },
            }
        };
        let package = manifest
            .app
            .package
            .clone()
            .or_else(|| {
                cargo
                    .get("package")?
                    .get("name")?
                    .as_str()
                    .map(str::to_owned)
            })
            .context("Set app.package in Omen.toml for a Cargo workspace")?;
        validate_name(&package)?;
        manifest.app.package = Some(package.clone());
        manifest.validate()?;
        let configured = manifest.toolchain.sdk.as_ref().map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                root.join(p)
            }
        });
        let sdk = sdk_path(configured.as_deref())?;
        let target = if let Some(configured) = std::env::var_os("CARGO_TARGET_DIR") {
            let path = PathBuf::from(configured);
            if path.is_absolute() {
                path
            } else {
                root.join(path)
            }
        } else {
            let output = std::process::Command::new(crate::tools::required("cargo")?)
                .current_dir(&root)
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .output()
                .context("Could not read Cargo target-directory configuration")?;
            if !output.status.success() {
                bail!(
                    "Cargo metadata failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
            let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            PathBuf::from(
                metadata["target_directory"]
                    .as_str()
                    .context("Cargo metadata omitted target_directory")?,
            )
        };
        Ok(Self {
            root,
            sdk,
            manifest,
            package,
            target,
        })
    }
    pub fn packaging_config(&self) -> Result<PathBuf> {
        let dir = self.root.join(".omen");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("app.json");
        let permissions: Vec<String> =
            if !self.root.join("Omen.toml").exists() && self.root.join("omen.app.json").exists() {
                let legacy: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(self.root.join("omen.app.json"))?)?;
                serde_json::from_value(
                    legacy
                        .get("permissions")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!([])),
                )?
            } else {
                Vec::new()
            };
        let config = serde_json::json!({"package":self.package,"native_package":self.manifest.app.native_package.as_ref().unwrap_or(&self.package),"name":self.manifest.app.name,"identifier":self.manifest.app.id,"version":self.manifest.app.version,"window":self.manifest.window,"permissions":permissions,"targets":self.manifest.targets.platforms});
        std::fs::write(&path, serde_json::to_vec_pretty(&config)?)?;
        Ok(path)
    }
}
