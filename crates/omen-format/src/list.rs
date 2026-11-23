//! Locale-aware list formatting for conjunctions, disjunctions, and units.
use crate::context::FormatContext;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// List combination type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListType {
    #[default]
    Conjunction, // "Alice, Bob, and Charlie"
    Disjunction, // "Alice, Bob, or Charlie"
    Unit,        // "1 m, 2 cm"
}

/// List display style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListStyle {
    #[default]
    Standard,
    Short,
    Narrow,
}

/// Options configuring list formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOptions {
    pub list_type: ListType,
    pub style: ListStyle,
}

impl ListOptions {
    pub fn conjunction() -> Self {
        Self { list_type: ListType::Conjunction, ..Self::default() }
    }

    pub fn disjunction() -> Self {
        Self { list_type: ListType::Disjunction, ..Self::default() }
    }

    pub fn unit() -> Self {
        Self { list_type: ListType::Unit, ..Self::default() }
    }
}

/// Format a slice of items into a localized string list.
pub fn format_list(
    items: &[impl AsRef<str>],
    options: &ListOptions,
    context: &FormatContext,
) -> Result<String> {
    if items.is_empty() {
        return Ok(String::new());
    }
    if items.len() == 1 {
        return Ok(items[0].as_ref().to_string());
    }

    let lang = context.locale.language();
    let strings: Vec<&str> = items.iter().map(|s| s.as_ref()).collect();

    let (conj_word, disj_word, use_oxford) = match lang {
        "de" => ("und", "oder", false),
        "fr" => ("et", "ou", false),
        "es" => ("y", "o", false),
        "it" => ("e", "o", false),
        "hi" => ("और", "या", false),
        "ar" => ("و", "أو", false),
        _ => ("and", "or", true),
    };

    let word = match options.list_type {
        ListType::Conjunction => conj_word,
        ListType::Disjunction => disj_word,
        ListType::Unit => return Ok(strings.join(", ")),
    };

    if lang == "ja" {
        return Ok(strings.join("、"));
    }

    if strings.len() == 2 {
        return Ok(format!("{} {word} {}", strings[0], strings[1]));
    }

    let len = strings.len();
    let initial = strings[..len - 1].join(", ");
    let last = strings[len - 1];

    let output = if use_oxford {
        format!("{initial}, {word} {last}")
    } else {
        format!("{initial} {word} {last}")
    };

    Ok(output)
}
