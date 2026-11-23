//! Exact decimal representation and rounding modes for financial and precision formatting.
use crate::error::{FormatError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Rounding strategies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoundingMode {
    #[default]
    HalfEven, // Banker's rounding (round to nearest even)
    HalfUp,   // Traditional school rounding (.5 rounds up away from zero)
    TowardZero, // Truncation
    AwayFromZero, // Round away from zero
    Floor,    // Towards -infinity
    Ceiling,  // Towards +infinity
}

/// An exact decimal value stored without binary floating-point representation errors.
/// Supports up to 28 decimal places of exact precision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Decimal {
    /// Scaled integer value: mantissa / 10^scale
    mantissa: i128,
    scale: u32,
}

impl Decimal {
    pub const ZERO: Self = Self { mantissa: 0, scale: 0 };
    pub const ONE: Self = Self { mantissa: 1, scale: 0 };

    pub const fn from_parts(mantissa: i128, scale: u32) -> Self {
        Self { mantissa, scale }
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa == 0
    }

    pub fn is_negative(&self) -> bool {
        self.mantissa < 0
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn mantissa(&self) -> i128 {
        self.mantissa
    }

    pub fn abs(self) -> Self {
        Self {
            mantissa: self.mantissa.abs(),
            scale: self.scale,
        }
    }

    pub fn negate(self) -> Self {
        Self {
            mantissa: -self.mantissa,
            scale: self.scale,
        }
    }

    /// Rescale to target fraction digits with the chosen rounding mode.
    pub fn round_to_scale(&self, target_scale: u32, mode: RoundingMode) -> Self {
        if self.scale == target_scale {
            return *self;
        }

        if self.scale < target_scale {
            let diff = target_scale - self.scale;
            let factor = 10i128.pow(diff);
            return Self {
                mantissa: self.mantissa * factor,
                scale: target_scale,
            };
        }

        let diff = self.scale - target_scale;
        let factor = 10i128.pow(diff);
        let quotient = self.mantissa / factor;
        let remainder = self.mantissa % factor;

        if remainder == 0 {
            return Self {
                mantissa: quotient,
                scale: target_scale,
            };
        }

        let is_neg = self.mantissa < 0;
        let abs_rem = remainder.abs();
        let half = factor / 2;

        let should_round_up = match mode {
            RoundingMode::TowardZero => false,
            RoundingMode::AwayFromZero => true,
            RoundingMode::Floor => is_neg,
            RoundingMode::Ceiling => !is_neg,
            RoundingMode::HalfUp => abs_rem >= half,
            RoundingMode::HalfEven => {
                if abs_rem > half {
                    true
                } else if abs_rem < half {
                    false
                } else {
                    // Exactly half -> round so quotient is even
                    quotient.abs() % 2 != 0
                }
            }
        };

        let new_mantissa = if should_round_up {
            if is_neg {
                quotient - 1
            } else {
                quotient + 1
            }
        } else {
            quotient
        };

        Self {
            mantissa: new_mantissa,
            scale: target_scale,
        }
    }

    /// Split into sign, integer part string, and fractional part string.
    pub fn to_parts(&self, fraction_digits: usize) -> (bool, String, String) {
        let rounded = self.round_to_scale(fraction_digits as u32, RoundingMode::HalfEven);
        let is_neg = rounded.mantissa < 0;
        let abs_m = rounded.mantissa.abs();
        let factor = 10i128.pow(fraction_digits as u32);

        let int_val = if fraction_digits == 0 {
            abs_m
        } else {
            abs_m / factor
        };
        let frac_val = if fraction_digits == 0 {
            0
        } else {
            abs_m % factor
        };

        let int_str = int_val.to_string();
        let frac_str = if fraction_digits > 0 {
            format!("{:0width$}", frac_val, width = fraction_digits)
        } else {
            String::new()
        };

        (is_neg, int_str, frac_str)
    }

    pub fn to_f64(&self) -> f64 {
        self.mantissa as f64 / 10f64.powi(self.scale as i32)
    }
}

impl Default for Decimal {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Debug for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decimal({})", self)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (is_neg, int_part, frac_part) = self.to_parts(self.scale as usize);
        if is_neg {
            write!(f, "-")?;
        }
        write!(f, "{int_part}")?;
        if self.scale > 0 {
            write!(f, ".{frac_part}")?;
        }
        Ok(())
    }
}

impl FromStr for Decimal {
    type Err = FormatError;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(FormatError::ParseError("empty decimal string".into()));
        }

        let is_neg = s.starts_with('-');
        let rest = s.trim_start_matches(['+', '-']);

        let mut parts = rest.split('.');
        let int_part = parts.next().unwrap_or("0");
        let frac_part = parts.next();

        if parts.next().is_some() {
            return Err(FormatError::ParseError("multiple decimal points".into()));
        }

        let int_digits: i128 = if int_part.is_empty() {
            0
        } else {
            int_part
                .parse()
                .map_err(|e| FormatError::ParseError(format!("invalid integer digits: {e}")))?
        };

        if let Some(frac) = frac_part {
            let scale = frac.len() as u32;
            let factor = 10i128.pow(scale);
            let frac_digits: i128 = if frac.is_empty() {
                0
            } else {
                frac.parse()
                    .map_err(|e| FormatError::ParseError(format!("invalid fractional digits: {e}")))?
            };

            let mantissa = int_digits * factor + frac_digits;
            let signed_mantissa = if is_neg { -mantissa } else { mantissa };
            Ok(Self {
                mantissa: signed_mantissa,
                scale,
            })
        } else {
            let signed_mantissa = if is_neg { -int_digits } else { int_digits };
            Ok(Self {
                mantissa: signed_mantissa,
                scale: 0,
            })
        }
    }
}

macro_rules! impl_from_int {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Decimal {
                fn from(v: $t) -> Self {
                    Self {
                        mantissa: v as i128,
                        scale: 0,
                    }
                }
            }
        )*
    };
}

impl_from_int!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, usize);

impl From<f64> for Decimal {
    fn from(f: f64) -> Self {
        // Format to string with 6 decimal places to prevent float rounding noise
        let s = format!("{f:.6}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        Self::from_str(trimmed).unwrap_or(Self::ZERO)
    }
}

impl From<f32> for Decimal {
    fn from(f: f32) -> Self {
        Self::from(f as f64)
    }
}
