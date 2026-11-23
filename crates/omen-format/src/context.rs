//! Typed formatting context describing locale, time zone, calendar, and preferences.
use serde::{Deserialize, Serialize};
use std::fmt;

/// A validated BCP-47 locale identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocaleId(String);

impl LocaleId {
    pub fn new(id: impl Into<String>) -> Self {
        let s = id.into().replace('_', "-");
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn language(&self) -> &str {
        self.0.split('-').next().unwrap_or("en")
    }

    pub fn region(&self) -> Option<&str> {
        let mut parts = self.0.split('-');
        let _lang = parts.next()?;
        parts.next()
    }
}

impl Default for LocaleId {
    fn default() -> Self {
        Self("en-US".into())
    }
}

impl fmt::Display for LocaleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: Into<String>> From<S> for LocaleId {
    fn from(s: S) -> Self {
        Self::new(s)
    }
}

/// Standard or named time zone identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeZoneId {
    Utc,
    Offset(i32), // Offset in seconds from UTC
    Named(String),
}

impl TimeZoneId {
    pub fn utc() -> Self {
        Self::Utc
    }

    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        if name.eq_ignore_ascii_case("utc") || name.eq_ignore_ascii_case("gmt") {
            Self::Utc
        } else {
            Self::Named(name)
        }
    }

    pub fn offset_hours(hours: i32) -> Self {
        Self::Offset(hours * 3600)
    }

    pub fn offset_seconds(seconds: i32) -> Self {
        Self::Offset(seconds)
    }

    /// Fixed offset in seconds if known directly, or approximation for common named zones.
    pub fn fixed_offset_seconds(&self) -> i32 {
        match self {
            Self::Utc => 0,
            Self::Offset(sec) => *sec,
            Self::Named(name) => match name.as_str() {
                "Asia/Kolkata" | "Asia/Calcutta" | "IST" => 19800, // +05:30
                "Asia/Tokyo" | "JST" => 32400,                    // +09:00
                "Europe/London" | "GMT" | "WET" => 0,
                "Europe/Paris" | "Europe/Berlin" | "CET" => 3600, // +01:00
                "America/New_York" | "EST" => -18000,             // -05:00
                "America/Chicago" | "CST" => -21600,              // -06:00
                "America/Denver" | "MST" => -25200,               // -07:00
                "America/Los_Angeles" | "PST" => -28800,          // -08:00
                "Australia/Sydney" | "AEST" => 36000,             // +10:00
                "Pacific/Auckland" | "NZST" => 43200,             // +12:00
                "Asia/Dubai" | "GST" => 14400,                    // +04:00
                "Asia/Singapore" | "SGT" => 28800,                // +08:00
                _ => 0,
            },
        }
    }
}

impl<S: Into<String>> From<S> for TimeZoneId {
    fn from(name: S) -> Self {
        Self::named(name)
    }
}

impl Default for TimeZoneId {
    fn default() -> Self {
        Self::Utc
    }
}

impl fmt::Display for TimeZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utc => write!(f, "UTC"),
            Self::Offset(sec) => {
                let sign = if *sec >= 0 { '+' } else { '-' };
                let abs = sec.abs();
                let hours = abs / 3600;
                let mins = (abs % 3600) / 60;
                write!(f, "UTC{sign}{hours:02}:{mins:02}")
            }
            Self::Named(name) => write!(f, "{name}"),
        }
    }
}

/// Calendar system identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CalendarId {
    #[default]
    Gregorian,
    Buddhist,
    Islamic,
    Japanese,
    Persian,
    Hebrew,
    Indian,
}

/// Numbering system identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NumberingSystem {
    #[default]
    Latn, // Latin digits 0-9
    Arab, // Arabic-Indic digits ٠-٩
    ArabExt, // Extended Arabic-Indic digits
    Deva, // Devanagari digits ०-९
    Hans, // Simplified Chinese
    Hant, // Traditional Chinese
    Fullwide,
}

impl NumberingSystem {
    /// Transcribe ASCII digits 0-9 into the target numbering system.
    pub fn transcribe_digits(self, text: &str) -> String {
        match self {
            Self::Latn => text.to_string(),
            Self::Arab => text
                .chars()
                .map(|c| match c {
                    '0'..='9' => char::from_u32(0x0660 + (c as u32 - '0' as u32)).unwrap_or(c),
                    '.' => '٫',
                    ',' => '٬',
                    _ => c,
                })
                .collect(),
            Self::Deva => text
                .chars()
                .map(|c| match c {
                    '0'..='9' => char::from_u32(0x0966 + (c as u32 - '0' as u32)).unwrap_or(c),
                    _ => c,
                })
                .collect(),
            _ => text.to_string(),
        }
    }
}

/// Hour cycle preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HourCycle {
    #[default]
    Auto,
    H11, // 0-11 (12h starting at 0)
    H12, // 1-12 (12h standard am/pm)
    H23, // 0-23 (24h standard)
    H24, // 1-24 (24h starting at 1)
}

/// Measurement system preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MeasurementSystem {
    #[default]
    Metric,
    Imperial,
    UsCustomary,
}

/// ISO 4217 3-letter currency code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CurrencyCode([u8; 3]);

impl CurrencyCode {
    pub const USD: Self = Self(*b"USD");
    pub const EUR: Self = Self(*b"EUR");
    pub const INR: Self = Self(*b"INR");
    pub const GBP: Self = Self(*b"GBP");
    pub const JPY: Self = Self(*b"JPY");
    pub const CNY: Self = Self(*b"CNY");
    pub const CAD: Self = Self(*b"CAD");
    pub const AUD: Self = Self(*b"AUD");
    pub const CHF: Self = Self(*b"CHF");
    pub const BRL: Self = Self(*b"BRL");

    pub fn new(code: &str) -> Option<Self> {
        let bytes = code.as_bytes();
        if bytes.len() == 3 && bytes.iter().all(|b| b.is_ascii_alphabetic()) {
            let mut upper = [0u8; 3];
            for i in 0..3 {
                upper[i] = bytes[i].to_ascii_uppercase();
            }
            Some(Self(upper))
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("USD")
    }

    /// Standard fraction digits for this currency.
    pub fn standard_fraction_digits(self) -> usize {
        match self.as_str() {
            "JPY" | "KRW" | "CLP" | "VND" | "HUF" | "TWD" | "ISK" => 0,
            "BHD" | "KWD" | "OMR" | "JOD" | "TND" => 3,
            _ => 2,
        }
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Complete formatting context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatContext {
    pub locale: LocaleId,
    pub time_zone: TimeZoneId,
    pub calendar: CalendarId,
    pub numbering_system: NumberingSystem,
    pub hour_cycle: HourCycle,
    pub measurement_system: MeasurementSystem,
    pub currency: Option<CurrencyCode>,
}

impl Default for FormatContext {
    fn default() -> Self {
        Self {
            locale: LocaleId::default(),
            time_zone: TimeZoneId::default(),
            calendar: CalendarId::default(),
            numbering_system: NumberingSystem::default(),
            hour_cycle: HourCycle::default(),
            measurement_system: MeasurementSystem::default(),
            currency: None,
        }
    }
}

impl FormatContext {
    pub fn new(locale: impl Into<LocaleId>) -> Self {
        let locale = locale.into();
        let measurement_system = match locale.region() {
            Some("US") => MeasurementSystem::UsCustomary,
            Some("GB") => MeasurementSystem::Imperial,
            _ => MeasurementSystem::Metric,
        };
        let numbering_system = match locale.language() {
            "ar" => NumberingSystem::Arab,
            "hi" => NumberingSystem::Latn, // Modern Hindi UI typically uses Latin or Deva depending on config
            _ => NumberingSystem::Latn,
        };
        let currency = match locale.region() {
            Some("US") => CurrencyCode::new("USD"),
            Some("GB") => CurrencyCode::new("GBP"),
            Some("IN") => CurrencyCode::new("INR"),
            Some("DE") | Some("FR") | Some("IT") | Some("ES") => CurrencyCode::new("EUR"),
            Some("JP") => CurrencyCode::new("JPY"),
            _ => None,
        };
        Self {
            locale,
            time_zone: TimeZoneId::Utc,
            calendar: CalendarId::Gregorian,
            numbering_system,
            hour_cycle: HourCycle::Auto,
            measurement_system,
            currency,
        }
    }

    pub fn with_time_zone(mut self, time_zone: impl Into<TimeZoneId>) -> Self {
        self.time_zone = time_zone.into();
        self
    }

    pub fn with_currency(mut self, currency: CurrencyCode) -> Self {
        self.currency = Some(currency);
        self
    }

    pub fn with_numbering_system(mut self, numbering_system: NumberingSystem) -> Self {
        self.numbering_system = numbering_system;
        self
    }

    pub fn with_hour_cycle(mut self, hour_cycle: HourCycle) -> Self {
        self.hour_cycle = hour_cycle;
        self
    }

    pub fn with_measurement_system(mut self, measurement_system: MeasurementSystem) -> Self {
        self.measurement_system = measurement_system;
        self
    }
}
