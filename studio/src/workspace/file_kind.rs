//! Central classification, icon and starter-template registry.
use std::{io, path::Path};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StudioFileKind {
    View,
    State,
    Environment,
    Localization,
    Module,
    Rust,
    Folder,
    #[default]
    Other,
}
impl StudioFileKind {
    pub const CREATABLE: [Self; 7] = [
        Self::View,
        Self::State,
        Self::Environment,
        Self::Localization,
        Self::Module,
        Self::Folder,
        Self::Rust,
    ];
    pub fn classify(path: &Path, directory: bool) -> Self {
        if directory {
            return Self::Folder;
        }
        match path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
        {
            "mod.rs" => Self::Module,
            name if name.ends_with("_view.rs") => Self::View,
            name if name.ends_with("_state.rs") => Self::State,
            name if name.ends_with("_env.rs") => Self::Environment,
            name if name.ends_with("_locale.rs") => Self::Localization,
            name if name.ends_with(".rs") => Self::Rust,
            _ => Self::Other,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "New Omen View",
            Self::State => "New Omen State",
            Self::Environment => "New Environment",
            Self::Localization => "New Localization",
            Self::Module => "New Rust Module",
            Self::Folder => "New Folder",
            Self::Rust => "New Rust File",
            Self::Other => "New File",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::View => "Omen View",
            Self::State => "Omen State",
            Self::Environment => "Environment",
            Self::Localization => "Localization",
            Self::Module => "Rust Module",
            Self::Folder => "Folder",
            Self::Rust => "Rust File",
            Self::Other => "Plain File",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::View => "A declarative view! component with a starter layout.",
            Self::State => "Reactive state and actions using Omen's state! macro.",
            Self::Environment => "Shared values injected through an Omen environment.",
            Self::Localization => "A locale! catalog for translated application strings.",
            Self::Module => "A module folder with mod.rs and parent registration.",
            Self::Folder => "An empty folder for organizing project files.",
            Self::Rust => "An ordinary .rs source file with optional module registration.",
            Self::Other => "An empty file with a name and extension of your choice.",
        }
    }
    pub fn matches(self, query: &str) -> bool {
        self.label().to_lowercase().contains(query)
            || self.description().to_lowercase().contains(query)
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::View => "file-omen-view",
            Self::State => "file-omen-state",
            Self::Environment => "file-omen-env",
            Self::Localization => "file-omen-locale",
            Self::Module => "file-rust-module",
            Self::Rust => "file-rust",
            Self::Folder => "folder",
            Self::Other => "file",
        }
    }
    pub fn tint(self, dark: bool) -> u32 {
        match (self, dark) {
            (Self::View, true) => 0x89baff,
            (Self::View, false) => 0x2365ad,
            (Self::State, true) => 0xcba6f7,
            (Self::State, false) => 0x8351b5,
            (Self::Environment, true) => 0x81d4c0,
            (Self::Environment, false) => 0x187e69,
            (Self::Localization, true) => 0xf2c078,
            (Self::Localization, false) => 0x936113,
            (Self::Module, true) => 0xe7a0bc,
            (Self::Module, false) => 0xa03d68,
            (Self::Rust, _) => 0xe97932,
            (_, true) => 0x8096ad,
            (_, false) => 0x526b85,
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::View => "_view",
            Self::State => "_state",
            Self::Environment => "_env",
            Self::Localization => "_locale",
            _ => "",
        }
    }
    pub fn template(self) -> &'static str {
        match self {
            Self::View => include_str!("../../templates/view.rs.tpl"),
            Self::State => include_str!("../../templates/state.rs.tpl"),
            Self::Environment => include_str!("../../templates/env.rs.tpl"),
            Self::Localization => include_str!("../../templates/locale.rs.tpl"),
            Self::Module => include_str!("../../templates/module.rs.tpl"),
            _ => "// {{type_name}}\n",
        }
    }
}
#[derive(Clone, Debug)]
pub struct FileNames {
    pub file_name: String,
    pub module_name: String,
    pub type_name: String,
    pub view_name: String,
}
impl FileNames {
    pub fn new(name: &str, kind: StudioFileKind) -> io::Result<Self> {
        if kind == StudioFileKind::Folder {
            let name = name.trim();
            super::validate_entry_name(name)?;
            return Ok(Self {
                file_name: name.into(),
                module_name: name.into(),
                type_name: String::new(),
                view_name: String::new(),
            });
        }
        let name = name.trim().strip_suffix(".rs").unwrap_or(name.trim());
        let name = if kind.suffix().is_empty() {
            name
        } else {
            name.strip_suffix(kind.suffix()).unwrap_or(name)
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' '))
        {
            return Err(io::Error::other(
                "Use letters, digits, spaces, underscores or hyphens; paths and punctuation are not allowed",
            ));
        }
        let chars: Vec<_> = name.chars().collect();
        let mut snake = String::new();
        for (index, ch) in chars.iter().copied().enumerate() {
            if matches!(ch, '_' | '-' | ' ') {
                if !snake.is_empty() && !snake.ends_with('_') {
                    snake.push('_');
                }
                continue;
            }
            if ch.is_uppercase()
                && !snake.is_empty()
                && !snake.ends_with('_')
                && (index > 0 && chars[index - 1].is_lowercase()
                    || chars.get(index + 1).is_some_and(|ch| ch.is_lowercase())
                        && index > 0
                        && chars[index - 1].is_uppercase())
            {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
        }
        let base = snake.trim_matches('_');
        let base = if kind.suffix().is_empty() {
            base
        } else {
            base.strip_suffix(kind.suffix()).unwrap_or(base)
        };
        if base.is_empty() || base.starts_with(|ch: char| ch.is_ascii_digit()) {
            return Err(io::Error::other(
                "The name must start with a letter and cannot be a Rust keyword",
            ));
        }
        let view_name = base
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| format!("{}{}", part[..1].to_ascii_uppercase(), &part[1..]))
            .collect::<String>();
        let module_name = format!("{base}{}", kind.suffix());
        let type_name = format!(
            "{view_name}{}",
            match kind {
                StudioFileKind::State => "State",
                StudioFileKind::Environment => "Environment",
                StudioFileKind::Localization => "Locale",
                _ => "",
            }
        );
        if syn::parse_str::<syn::Ident>(&module_name).is_err()
            || matches!(
                kind,
                StudioFileKind::View
                    | StudioFileKind::State
                    | StudioFileKind::Environment
                    | StudioFileKind::Localization
            ) && syn::parse_str::<syn::Ident>(&type_name).is_err()
        {
            return Err(io::Error::other(
                "This name would create a reserved or invalid Rust identifier",
            ));
        }
        let file_name = if kind == StudioFileKind::Module {
            format!("{module_name}/mod.rs")
        } else if kind == StudioFileKind::Folder {
            module_name.clone()
        } else {
            format!("{module_name}.rs")
        };
        super::validate_entry_name(&module_name)?;
        Ok(Self {
            file_name,
            module_name,
            type_name,
            view_name,
        })
    }
    pub fn render(&self, kind: StudioFileKind) -> String {
        kind.template()
            .replace("{{file_name}}", &self.file_name)
            .replace("{{module_name}}", &self.module_name)
            .replace("{{type_name}}", &self.type_name)
            .replace("{{view_name}}", &self.view_name)
    }
}
