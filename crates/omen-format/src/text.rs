//! Unicode-aware text transformations, casing, and grapheme truncation.
use crate::context::FormatContext;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// Text transformation kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextTransform {
    Uppercase,
    Lowercase,
    TitleCase,
    SentenceCase,
    Capitalize,
    Trim,
    NormalizeWhitespace,
    Truncate(usize), // Truncate at grapheme cluster boundary with ellipsis '…'
}

/// Options configuring text transformation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextOptions {
    pub transform: TextTransform,
}

impl TextOptions {
    pub fn uppercase() -> Self {
        Self { transform: TextTransform::Uppercase }
    }

    pub fn lowercase() -> Self {
        Self { transform: TextTransform::Lowercase }
    }

    pub fn title_case() -> Self {
        Self { transform: TextTransform::TitleCase }
    }

    pub fn capitalize() -> Self {
        Self { transform: TextTransform::Capitalize }
    }

    pub fn truncate(max_graphemes: usize) -> Self {
        Self { transform: TextTransform::Truncate(max_graphemes) }
    }

    pub fn trim() -> Self {
        Self { transform: TextTransform::Trim }
    }
}

/// Format/transform text with Unicode safety and locale sensitivity.
pub fn format_text(
    text: &str,
    options: &TextOptions,
    context: &FormatContext,
) -> Result<String> {
    let lang = context.locale.language();

    let output = match options.transform {
        TextTransform::Uppercase => {
            if lang == "tr" || lang == "az" {
                // Turkish dotted/dotless i: 'i' -> 'İ', 'ı' -> 'I'
                text.chars()
                    .map(|c| match c {
                        'i' => 'İ',
                        'ı' => 'I',
                        _ => c.to_uppercase().next().unwrap_or(c),
                    })
                    .collect()
            } else {
                text.to_uppercase()
            }
        }
        TextTransform::Lowercase => {
            if lang == "tr" || lang == "az" {
                // Turkish dotted/dotless i: 'I' -> 'ı', 'İ' -> 'i'
                text.chars()
                    .map(|c| match c {
                        'I' => 'ı',
                        'İ' => 'i',
                        _ => c.to_lowercase().next().unwrap_or(c),
                    })
                    .collect()
            } else {
                text.to_lowercase()
            }
        }
        TextTransform::Capitalize => {
            let mut chars = text.chars();
            match chars.next() {
                Some(first) => {
                    let upper = first.to_uppercase();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        }
        TextTransform::TitleCase => {
            let mut result = String::new();
            for word in text.split_whitespace() {
                if !result.is_empty() {
                    result.push(' ');
                }
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    result.extend(first.to_uppercase());
                    for rest in chars {
                        result.extend(rest.to_lowercase());
                    }
                }
            }
            result
        }
        TextTransform::SentenceCase => {
            let mut result = String::new();
            let mut capitalize_next = true;
            for ch in text.chars() {
                if capitalize_next && ch.is_alphabetic() {
                    result.extend(ch.to_uppercase());
                    capitalize_next = false;
                } else {
                    result.extend(ch.to_lowercase());
                    if ch == '.' || ch == '!' || ch == '?' {
                        capitalize_next = true;
                    }
                }
            }
            result
        }
        TextTransform::Trim => text.trim().to_string(),
        TextTransform::NormalizeWhitespace => {
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        TextTransform::Truncate(max_graphemes) => {
            let graphemes: Vec<&str> = text.graphemes(true).collect();
            if graphemes.len() <= max_graphemes {
                text.to_string()
            } else {
                let truncated: String = graphemes[..max_graphemes].concat();
                format!("{truncated}…")
            }
        }
    };

    Ok(output)
}
