//! Plural rules and localized ordinal formatting.
use crate::context::FormatContext;
use crate::error::Result;
use intl_pluralrules::{PluralCategory as IPluralCategory, PluralRuleType, PluralRules};
use serde::{Deserialize, Serialize};

/// CLDR Plural Category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// Select the cardinal plural category for a count and locale.
pub fn select_cardinal(count: i64, locale: &str) -> PluralCategory {
    let locale_str = locale.replace('_', "-");
    let language = locale_str.split('-').next().unwrap_or("en");

    let cat = [locale_str.as_str(), language].into_iter().find_map(|id| {
        let parsed = id.parse::<unic_langid::LanguageIdentifier>().ok()?;
        PluralRules::create(parsed, PluralRuleType::CARDINAL)
            .ok()?
            .select(count.unsigned_abs())
            .ok()
    });

    match cat {
        Some(IPluralCategory::ZERO) => PluralCategory::Zero,
        Some(IPluralCategory::ONE) => PluralCategory::One,
        Some(IPluralCategory::TWO) => PluralCategory::Two,
        Some(IPluralCategory::FEW) => PluralCategory::Few,
        Some(IPluralCategory::MANY) => PluralCategory::Many,
        _ => PluralCategory::Other,
    }
}

/// Format an ordinal number (e.g. 1 -> "1st", 2 -> "2nd", 3 -> "3rd").
pub fn format_ordinal(number: i64, context: &FormatContext) -> Result<String> {
    let lang = context.locale.language();
    let num_str = number.to_string();

    let output = match lang {
        "en" => {
            let abs = number.abs();
            let tens = abs % 100;
            let units = abs % 10;
            let suffix = if tens >= 11 && tens <= 13 {
                "th"
            } else {
                match units {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                }
            };
            format!("{num_str}{suffix}")
        }
        "fr" => {
            if number == 1 {
                format!("{num_str}er")
            } else {
                format!("{num_str}e")
            }
        }
        "de" => format!("{num_str}."),
        "es" => format!("{num_str}.º"),
        "ja" => format!("第{num_str}"),
        _ => format!("{num_str}th"),
    };

    Ok(output)
}
