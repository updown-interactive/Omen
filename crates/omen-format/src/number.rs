//! Number formatting with locale-aware decimal and grouping separators, precision, and notations.
use crate::context::{FormatContext, NumberingSystem};
use crate::decimal::{Decimal, RoundingMode};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Sign display strategy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignDisplay {
    #[default]
    Auto, // Minus for negative, none for positive
    Always, // Explicit + and -
    Never,  // Absolute value
    ExceptZero, // + and - unless zero
}

/// Number notation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumberNotation {
    #[default]
    Standard,
    Scientific,
    Engineering,
    Compact,
}

/// Options configuring number formatting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberOptions {
    pub minimum_integer_digits: usize,
    pub minimum_fraction_digits: usize,
    pub maximum_fraction_digits: usize,
    pub use_grouping: bool,
    pub sign_display: SignDisplay,
    pub notation: NumberNotation,
    pub rounding_mode: RoundingMode,
}

impl Default for NumberOptions {
    fn default() -> Self {
        Self {
            minimum_integer_digits: 1,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 3,
            use_grouping: true,
            sign_display: SignDisplay::Auto,
            notation: NumberNotation::Standard,
            rounding_mode: RoundingMode::HalfEven,
        }
    }
}

impl NumberOptions {
    pub fn standard() -> Self {
        Self::default()
    }

    pub fn compact() -> Self {
        Self {
            notation: NumberNotation::Compact,
            maximum_fraction_digits: 1,
            ..Self::default()
        }
    }

    pub fn scientific() -> Self {
        Self {
            notation: NumberNotation::Scientific,
            ..Self::default()
        }
    }

    pub fn precision(mut self, min: usize, max: usize) -> Self {
        self.minimum_fraction_digits = min;
        self.maximum_fraction_digits = max;
        self
    }

    pub fn fixed_fraction(mut self, digits: usize) -> Self {
        self.minimum_fraction_digits = digits;
        self.maximum_fraction_digits = digits;
        self
    }

    pub fn grouping(mut self, use_grouping: bool) -> Self {
        self.use_grouping = use_grouping;
        self
    }

    pub fn sign(mut self, sign_display: SignDisplay) -> Self {
        self.sign_display = sign_display;
        self
    }
}

/// Locale-specific numeric symbols.
#[derive(Clone, Copy, Debug)]
pub struct LocaleNumberSymbols {
    pub decimal: &'static str,
    pub grouping: &'static str,
    pub is_indian_grouping: bool,
}

impl LocaleNumberSymbols {
    pub fn for_locale(locale: &str) -> Self {
        let lang = locale.split(['-', '_']).next().unwrap_or("en");
        let region = locale.split(['-', '_']).nth(1).unwrap_or("");

        if region.eq_ignore_ascii_case("IN") || lang.eq_ignore_ascii_case("hi") {
            Self {
                decimal: ".",
                grouping: ",",
                is_indian_grouping: true,
            }
        } else if matches!(lang, "de" | "it" | "nl" | "pt" | "es" | "da" | "no") {
            Self {
                decimal: ",",
                grouping: ".",
                is_indian_grouping: false,
            }
        } else if matches!(lang, "fr" | "ru" | "sv" | "fi" | "pl" | "cs") {
            Self {
                decimal: ",",
                grouping: "\u{00A0}", // Non-breaking space
                is_indian_grouping: false,
            }
        } else if lang == "ar" {
            Self {
                decimal: "٫",
                grouping: "٬",
                is_indian_grouping: false,
            }
        } else {
            Self {
                decimal: ".",
                grouping: ",",
                is_indian_grouping: false,
            }
        }
    }
}

/// Format a decimal number with explicit options and context.
pub fn format_number(
    value: impl Into<Decimal>,
    options: &NumberOptions,
    context: &FormatContext,
) -> Result<String> {
    let decimal: Decimal = value.into();
    let symbols = LocaleNumberSymbols::for_locale(context.locale.as_str());

    // Handle compact notation
    if options.notation == NumberNotation::Compact {
        return format_compact(decimal, options, context, &symbols);
    }

    // Handle scientific notation
    if options.notation == NumberNotation::Scientific {
        return format_scientific(decimal, options);
    }

    let is_neg = decimal.is_negative();
    let is_zero = decimal.is_zero();

    // Determine sign prefix
    let sign_prefix = match options.sign_display {
        SignDisplay::Auto => if is_neg { "-" } else { "" },
        SignDisplay::Always => if is_neg { "-" } else { "+" },
        SignDisplay::Never => "",
        SignDisplay::ExceptZero => {
            if is_zero {
                ""
            } else if is_neg {
                "-"
            } else {
                "+"
            }
        }
    };

    let fraction_digits = options
        .maximum_fraction_digits
        .max(options.minimum_fraction_digits);
    let (_, mut int_str, mut frac_str) = decimal.to_parts(fraction_digits);

    // Ensure minimum integer digits (leading zeros)
    if int_str.len() < options.minimum_integer_digits {
        let needed = options.minimum_integer_digits - int_str.len();
        int_str = format!("{}{int_str}", "0".repeat(needed));
    }

    // Grouping
    let grouped_int = if options.use_grouping {
        group_integer_digits(&int_str, symbols.grouping, symbols.is_indian_grouping)
    } else {
        int_str
    };

    // Trim trailing zeros down to minimum_fraction_digits
    if options.minimum_fraction_digits < options.maximum_fraction_digits {
        while frac_str.len() > options.minimum_fraction_digits && frac_str.ends_with('0') {
            frac_str.pop();
        }
    }

    let formatted_raw = if frac_str.is_empty() {
        format!("{sign_prefix}{grouped_int}")
    } else {
        format!("{sign_prefix}{grouped_int}{}{frac_str}", symbols.decimal)
    };

    // Numbering system
    let output = match context.numbering_system {
        NumberingSystem::Latn => formatted_raw,
        system => system.transcribe_digits(&formatted_raw),
    };

    Ok(output)
}

fn group_integer_digits(int_str: &str, sep: &str, indian: bool) -> String {
    if int_str.len() <= 3 {
        return int_str.to_string();
    }

    if indian {
        // Indian grouping: last 3 digits, then groups of 2 (e.g. 12,34,567)
        let len = int_str.len();
        let last3 = &int_str[len - 3..];
        let remaining = &int_str[..len - 3];
        let mut chunks = Vec::new();
        let mut r = remaining;
        while r.len() > 2 {
            let split_idx = r.len() - 2;
            chunks.push(&r[split_idx..]);
            r = &r[..split_idx];
        }
        if !r.is_empty() {
            chunks.push(r);
        }
        chunks.reverse();
        format!("{}{sep}{last3}", chunks.join(sep))
    } else {
        // Standard grouping: groups of 3 from the right
        let mut result = String::new();
        let chars: Vec<char> = int_str.chars().collect();
        let len = chars.len();
        for (i, &ch) in chars.iter().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push_str(sep);
            }
            result.push(ch);
        }
        result
    }
}

fn format_compact(
    decimal: Decimal,
    options: &NumberOptions,
    context: &FormatContext,
    _symbols: &LocaleNumberSymbols,
) -> Result<String> {
    let abs_val = decimal.to_f64().abs();
    let is_neg = decimal.is_negative();
    let sign = if is_neg { "-" } else { "" };

    let (scaled, suffix) = if abs_val >= 1_000_000_000_000.0 {
        (abs_val / 1_000_000_000_000.0, "T")
    } else if abs_val >= 1_000_000_000.0 {
        (abs_val / 1_000_000_000.0, "B")
    } else if abs_val >= 1_000_000.0 {
        (abs_val / 1_000_000.0, "M")
    } else if abs_val >= 1_000.0 {
        (abs_val / 1_000.0, "K")
    } else {
        (abs_val, "")
    };

    let scaled_dec = Decimal::from(scaled);
    let mut std_opts = options.clone();
    std_opts.notation = NumberNotation::Standard;
    let formatted_scaled = format_number(scaled_dec, &std_opts, context)?;

    Ok(format!("{sign}{formatted_scaled}{suffix}"))
}

fn format_scientific(decimal: Decimal, options: &NumberOptions) -> Result<String> {
    let f = decimal.to_f64();
    let prec = options.maximum_fraction_digits;
    Ok(format!("{f:.*e}", prec))
}
