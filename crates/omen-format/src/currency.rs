//! Currency formatting respecting ISO 4217, locale placement, and accounting notation.
use crate::context::{CurrencyCode, FormatContext};
use crate::decimal::Decimal;
use crate::error::Result;
use crate::number::{NumberOptions, format_number};
use serde::{Deserialize, Serialize};

/// Currency display style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CurrencyDisplay {
    #[default]
    Symbol, // "$", "€", "₹", "£"
    Code,   // "USD", "EUR", "INR"
    Name,   // "US dollars", "euros"
    NarrowSymbol, // "$"
}

/// Currency sign accounting style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CurrencySign {
    #[default]
    Standard, // -$10.00
    Accounting, // ($10.00)
}

/// Options configuring currency formatting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencyOptions {
    pub currency: CurrencyCode,
    pub display: CurrencyDisplay,
    pub sign: CurrencySign,
    pub fraction_digits: Option<usize>,
    pub use_grouping: bool,
}

impl CurrencyOptions {
    pub fn new(currency: CurrencyCode) -> Self {
        Self {
            currency,
            display: CurrencyDisplay::Symbol,
            sign: CurrencySign::Standard,
            fraction_digits: None,
            use_grouping: true,
        }
    }

    pub fn code(code: &str) -> Option<Self> {
        CurrencyCode::new(code).map(Self::new)
    }

    pub fn display(mut self, display: CurrencyDisplay) -> Self {
        self.display = display;
        self
    }

    pub fn accounting(mut self) -> Self {
        self.sign = CurrencySign::Accounting;
        self
    }

    pub fn fraction_digits(mut self, digits: usize) -> Self {
        self.fraction_digits = Some(digits);
        self
    }
}

/// A monetary amount with an associated currency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount: Decimal,
    pub currency: CurrencyCode,
}

impl Money {
    pub fn new(amount: impl Into<Decimal>, currency: CurrencyCode) -> Self {
        Self {
            amount: amount.into(),
            currency,
        }
    }

    pub fn usd(amount: impl Into<Decimal>) -> Self {
        Self::new(amount, CurrencyCode::USD)
    }

    pub fn eur(amount: impl Into<Decimal>) -> Self {
        Self::new(amount, CurrencyCode::EUR)
    }

    pub fn inr(amount: impl Into<Decimal>) -> Self {
        Self::new(amount, CurrencyCode::INR)
    }

    pub fn gbp(amount: impl Into<Decimal>) -> Self {
        Self::new(amount, CurrencyCode::GBP)
    }

    pub fn jpy(amount: impl Into<Decimal>) -> Self {
        Self::new(amount, CurrencyCode::JPY)
    }
}

/// Look up symbol for a currency.
pub fn currency_symbol(code: CurrencyCode) -> String {
    match code.as_str() {
        "USD" | "CAD" | "AUD" | "NZD" | "SGD" | "HKD" => "$".to_string(),
        "EUR" => "€".to_string(),
        "INR" => "₹".to_string(),
        "GBP" => "£".to_string(),
        "JPY" | "CNY" => "¥".to_string(),
        "BRL" => "R$".to_string(),
        "CHF" => "CHF".to_string(),
        "RUB" => "₽".to_string(),
        "KRW" => "₩".to_string(),
        "TRY" => "₺".to_string(),
        "THB" => "฿".to_string(),
        "ILS" => "₪".to_string(),
        _ => code.as_str().to_string(),
    }
}

/// Format money or amount + options.
pub fn format_currency(
    value: impl Into<Decimal>,
    options: &CurrencyOptions,
    context: &FormatContext,
) -> Result<String> {
    let decimal: Decimal = value.into();
    let is_neg = decimal.is_negative();
    let abs_decimal = decimal.abs();

    let digits = options
        .fraction_digits
        .unwrap_or_else(|| options.currency.standard_fraction_digits());

    let mut num_opts = NumberOptions::default();
    num_opts.minimum_fraction_digits = digits;
    num_opts.maximum_fraction_digits = digits;
    num_opts.use_grouping = options.use_grouping;

    let formatted_number = format_number(abs_decimal, &num_opts, context)?;

    let currency_label = match options.display {
        CurrencyDisplay::Symbol | CurrencyDisplay::NarrowSymbol => {
            currency_symbol(options.currency)
        }
        CurrencyDisplay::Code => options.currency.as_str().to_string(),
        CurrencyDisplay::Name => options.currency.as_str().to_string(),
    };

    let lang = context.locale.language();
    let region = context.locale.region().unwrap_or("");

    // Determine prefix vs suffix: In US, GB, IN, JP currency is prefix. In FR, DE, ES it is often suffix.
    let is_suffix = matches!(lang, "fr" | "de" | "sv" | "ru" | "it" | "es" | "pt")
        && !region.eq_ignore_ascii_case("US");

    let combined = if is_suffix {
        format!("{formatted_number}\u{00A0}{currency_label}")
    } else {
        // Prefix
        let space = if matches!(options.display, CurrencyDisplay::Code | CurrencyDisplay::Name) {
            "\u{00A0}"
        } else {
            ""
        };
        format!("{currency_label}{space}{formatted_number}")
    };

    let result = if is_neg {
        match options.sign {
            CurrencySign::Standard => {
                if is_suffix {
                    format!("-{combined}")
                } else {
                    format!("-{combined}")
                }
            }
            CurrencySign::Accounting => format!("({combined})"),
        }
    } else {
        combined
    };

    Ok(result)
}
