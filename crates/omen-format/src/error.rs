//! Structured formatting and parsing errors.

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    #[error("invalid locale identifier: {0}")]
    InvalidLocale(String),
    #[error("invalid time zone identifier: {0}")]
    InvalidTimeZone(String),
    #[error("invalid currency code: {0}")]
    InvalidCurrency(String),
    #[error("invalid calendar: {0}")]
    InvalidCalendar(String),
    #[error("invalid numbering system: {0}")]
    InvalidNumberingSystem(String),
    #[error("invalid formatting pattern: {0}")]
    InvalidPattern(String),
    #[error("unsupported value type: {0}")]
    UnsupportedType(&'static str),
    #[error("unsupported unit conversion from {from} to {to}")]
    InvalidUnitConversion { from: String, to: String },
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("missing localization key: {0}")]
    MissingKey(String),
    #[error("formatting failed: {0}")]
    Failed(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;
