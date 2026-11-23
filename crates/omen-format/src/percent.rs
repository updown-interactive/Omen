//! Percentage formatting with configurable scale, signs, and locale symbol placement.
use crate::context::FormatContext;
use crate::decimal::Decimal;
use crate::error::Result;
use crate::number::{NumberOptions, SignDisplay, format_number};
use serde::{Deserialize, Serialize};

/// Percentage input interpretation scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PercentScale {
    /// Values are fractions where 1.0 = 100%, 0.25 = 25%.
    #[default]
    Fraction,
    /// Values are already scaled where 25 = 25%, 100 = 100%.
    Whole,
}

/// Options configuring percentage formatting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentOptions {
    pub scale: PercentScale,
    pub minimum_fraction_digits: usize,
    pub maximum_fraction_digits: usize,
    pub sign_display: SignDisplay,
}

impl Default for PercentOptions {
    fn default() -> Self {
        Self {
            scale: PercentScale::Fraction,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 2,
            sign_display: SignDisplay::Auto,
        }
    }
}

impl PercentOptions {
    pub fn fraction() -> Self {
        Self {
            scale: PercentScale::Fraction,
            ..Self::default()
        }
    }

    pub fn whole() -> Self {
        Self {
            scale: PercentScale::Whole,
            ..Self::default()
        }
    }

    pub fn precision(mut self, min: usize, max: usize) -> Self {
        self.minimum_fraction_digits = min;
        self.maximum_fraction_digits = max;
        self
    }

    pub fn signed(mut self) -> Self {
        self.sign_display = SignDisplay::Always;
        self
    }
}

/// Format a value as a percentage.
pub fn format_percent(
    value: impl Into<Decimal>,
    options: &PercentOptions,
    context: &FormatContext,
) -> Result<String> {
    let mut decimal: Decimal = value.into();
    if options.scale == PercentScale::Fraction {
        // Multiply by 100
        decimal = Decimal::from_parts(decimal.mantissa() * 100, decimal.scale());
    }

    let mut num_opts = NumberOptions::default();
    num_opts.minimum_fraction_digits = options.minimum_fraction_digits;
    num_opts.maximum_fraction_digits = options.maximum_fraction_digits;
    num_opts.sign_display = options.sign_display;
    num_opts.use_grouping = false;

    let formatted_number = format_number(decimal, &num_opts, context)?;

    let lang = context.locale.language();
    let result = if lang == "tr" {
        // Turkish places percent symbol before the number
        format!("%{formatted_number}")
    } else if matches!(lang, "fr" | "de" | "ru" | "sv") {
        format!("{formatted_number}\u{00A0}%")
    } else {
        format!("{formatted_number}%")
    };

    Ok(result)
}
