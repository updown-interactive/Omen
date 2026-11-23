//! Format-aware headers for newly authored project files.
use chrono::Utc;
use omen::format::{DateTimeOptions, FormatContext, Instant, format};
use std::{fs, path::Path};

pub(super) struct FileHeader {
    app: String,
    created_at: String,
    author: String,
}

impl FileHeader {
    pub(super) fn new(app: &str, author: &str) -> Self {
        Self::with_context(app, author, &FormatContext::default())
    }

    pub(super) fn with_context(app: &str, author: &str, context: &FormatContext) -> Self {
        let now = Utc::now();
        let instant = Instant::from_epoch_seconds(now.timestamp());
        let options = DateTimeOptions::long();
        let created_at = format::instant(instant, &options, context)
            .unwrap_or_else(|_| now.to_rfc3339());

        Self {
            app: single_line(app),
            created_at,
            author: single_line(if author.trim().is_empty() {
                "Unknown"
            } else {
                author
            }),
        }
    }

    #[allow(dead_code)]
    pub(super) fn created_at(&self) -> &str {
        &self.created_at
    }

    pub(super) fn add(&self, path: &Path, source: &str) -> String {
        let Some(file) = path.file_name().and_then(|file| file.to_str()) else {
            return source.to_owned();
        };
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let (prefix, block) = match extension.as_str() {
            "rs" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "swift" | "go" | "js" | "jsx"
            | "mjs" | "ts" | "tsx" | "dart" | "cs" => ("// ", None),
            "toml" | "yaml" | "yml" | "py" | "rb" | "sh" | "bash" | "zsh" | "txt" => ("# ", None),
            "ini" => ("; ", None),
            "sql" | "lua" => ("-- ", None),
            "html" | "xml" | "svg" | "md" | "markdown" => ("  ", Some(("<!--", "-->", "--", "–"))),
            "css" | "scss" => (" * ", Some(("/*", " */", "*/", "* /"))),
            _ if matches!(
                file,
                ".gitignore" | ".gitattributes" | "Dockerfile" | "Makefile"
            ) =>
            {
                ("# ", None)
            }
            _ => return source.to_owned(),
        };
        let file = single_line(file);
        let fields = [
            ("File name", file.as_str()),
            ("App name", self.app.as_str()),
            ("Created at", self.created_at.as_str()),
            ("Created by", self.author.as_str()),
        ];
        let mut header = String::new();
        if let Some((opening, _, _, _)) = block {
            header.push_str(opening);
            header.push('\n');
        }
        for (label, value) in fields {
            header.push_str(prefix);
            header.push_str(label);
            header.push_str(": ");
            if let Some((_, _, forbidden, substitute)) = block {
                header.push_str(&value.replace(forbidden, substitute));
            } else {
                header.push_str(value);
            }
            header.push('\n');
        }
        if let Some((_, closing, _, _)) = block {
            header.push_str(closing);
            header.push('\n');
        }
        if !source.is_empty() {
            header.push('\n');
            header.push_str(source);
        }
        header
    }
}

pub(super) fn project_name(root: &Path) -> String {
    for (file, section) in [("Omen.toml", "app"), ("Cargo.toml", "package")] {
        if let Ok(source) = fs::read_to_string(root.join(file))
            && let Ok(manifest) = toml::from_str::<toml::Value>(&source)
            && let Some(name) = manifest
                .get(section)
                .and_then(|value| value.get("name"))
                .and_then(toml::Value::as_str)
            && !name.trim().is_empty()
        {
            return name.to_owned();
        }
    }
    root.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn single_line(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                ch
            }
        })
        .collect()
}
