//! International phone number formatting, canonical E.164, and region conventions.
use crate::context::FormatContext;
use crate::error::{FormatError, Result};
use serde::{Deserialize, Serialize};

/// Phone number display format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhoneFormat {
    #[default]
    International, // e.g. "+1 415 555 2671" or "+91 98765 43210"
    National,      // e.g. "(415) 555-2671" or "098765 43210"
    E164,          // e.g. "+14155552671"
}

/// Options configuring phone number formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneOptions {
    pub format: PhoneFormat,
    pub default_country_code: Option<u16>,
}

impl PhoneOptions {
    pub fn international() -> Self {
        Self { format: PhoneFormat::International, ..Self::default() }
    }

    pub fn national() -> Self {
        Self { format: PhoneFormat::National, ..Self::default() }
    }

    pub fn e164() -> Self {
        Self { format: PhoneFormat::E164, ..Self::default() }
    }

    pub fn default_country_code(mut self, code: u16) -> Self {
        self.default_country_code = Some(code);
        self
    }
}

/// A parsed phone number with country calling code and national number digits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneNumber {
    pub country_code: u16,
    pub national_number: String,
}

impl PhoneNumber {
    pub fn parse(input: &str, default_country: Option<u16>) -> Result<Self> {
        let cleaned: String = input
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect();

        if cleaned.is_empty() {
            return Err(FormatError::ParseError("empty phone number".into()));
        }

        if cleaned.starts_with('+') {
            let digits = &cleaned[1..];
            if digits.is_empty() {
                return Err(FormatError::ParseError("phone number has no digits after +".into()));
            }
            // Match known calling codes
            for (code, len) in [(1, 1), (44, 2), (91, 2), (49, 2), (33, 2), (81, 2), (86, 2), (61, 2), (55, 2)] {
                let code_str = code.to_string();
                if digits.starts_with(&code_str) {
                    return Ok(Self {
                        country_code: code,
                        national_number: digits[len..].to_string(),
                    });
                }
            }
            // Fallback: 1-digit or 2-digit code
            let cc: u16 = digits[..1].parse().unwrap_or(1);
            Ok(Self {
                country_code: cc,
                national_number: digits[1..].to_string(),
            })
        } else {
            let cc = default_country.unwrap_or(1);
            Ok(Self {
                country_code: cc,
                national_number: cleaned,
            })
        }
    }
}

/// Format a phone number according to options and context.
pub fn format_phone(
    number: &PhoneNumber,
    options: &PhoneOptions,
    _context: &FormatContext,
) -> Result<String> {
    let digits = &number.national_number;
    let cc = number.country_code;

    match options.format {
        PhoneFormat::E164 => Ok(format!("+{cc}{digits}")),
        PhoneFormat::International => match (cc, digits.len()) {
            // US/Canada: +1 AAA BBB CCCC
            (1, 10) => Ok(format!(
                "+1 {} {} {}",
                &digits[0..3],
                &digits[3..6],
                &digits[6..10]
            )),
            // India: +91 AAAAA BBBBB
            (91, 10) => Ok(format!("+91 {} {}", &digits[0..5], &digits[5..10])),
            // UK: +44 AAAA BBBBBB
            (44, 10) => Ok(format!("+44 {} {}", &digits[0..4], &digits[4..10])),
            // Germany: +49 AAA BBBBBBB
            (49, 10) => Ok(format!("+49 {} {}", &digits[0..3], &digits[3..10])),
            _ => Ok(format!("+{cc} {digits}")),
        },
        PhoneFormat::National => match (cc, digits.len()) {
            (1, 10) => Ok(format!(
                "({}) {}-{}",
                &digits[0..3],
                &digits[3..6],
                &digits[6..10]
            )),
            (91, 10) => Ok(format!("{} {}", &digits[0..5], &digits[5..10])),
            (44, 10) => Ok(format!("0{} {}", &digits[0..4], &digits[4..10])),
            (49, 10) => Ok(format!("0{} {}", &digits[0..3], &digits[3..10])),
            _ => Ok(digits.clone()),
        },
    }
}
