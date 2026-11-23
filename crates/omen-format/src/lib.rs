//! Omen Framework — Production Formatting & Localization Engine.
//!
//! Provides a cohesive, typed, cross-platform formatting architecture for:
//! - Dates and times (short, medium, long, full, year-only, custom patterns)
//! - Time zones and zoned date-times
//! - Relative times ("just now", "5 minutes ago", "yesterday", update boundaries)
//! - Durations (clock "01:32:08", human "1 hour, 32 minutes", short "1h 32m")
//! - Decimal precision & rounding modes
//! - Numbers with locale grouping and compact notation ("1.2K", "1.2M")
//! - Currencies with ISO 4217 symbols, placement, and accounting notation
//! - Percentages with fraction and whole scale
//! - Physical measurements and affine conversions
//! - File sizes (IEC binary vs SI decimal) and data rates
//! - Unicode text transformations and grapheme-aware truncation
//! - Conjunction and disjunction lists
//! - Pluralization and ordinal numbers
//! - Localized message interpolation with subformatting
//! - International and national phone numbers
//! - Country postal addresses
//! - Typed parsing and form validation
//! - AI agent metadata and Visual Editor Inspector integration

pub mod address;
pub mod bytes;
pub mod cache;
pub mod context;
pub mod currency;
pub mod custom;
pub mod date;
pub mod datetime;
pub mod decimal;
pub mod duration;
pub mod error;
pub mod list;
pub mod measurement;
pub mod message;
pub mod number;
pub mod options;
pub mod parse;
pub mod percent;
pub mod phone;
pub mod plural;
pub mod relative;
pub mod text;
pub mod time;
pub mod timezone;

pub use address::{AddressOptions, PostalAddress, format_address};
pub use bytes::{ByteOptions, ByteStandard, DataRateOptions, DataRateUnit, format_bytes, format_data_rate};
pub use cache::BoundedCache;
pub use context::{CalendarId, CurrencyCode, FormatContext, HourCycle, LocaleId, MeasurementSystem, NumberingSystem, TimeZoneId};
pub use currency::{CurrencyDisplay, CurrencyOptions, CurrencySign, Money, currency_symbol, format_currency};
pub use custom::{Formatter, FormatterDescriptor, standard_formatter_descriptors};
pub use date::{DateOptions, DateStyle, PlainDate, format_date};
pub use datetime::{DateTimeOptions, Instant, PlainDateTime, ZonedDateTime, format_datetime, format_instant};
pub use decimal::{Decimal, RoundingMode};
pub use duration::{DurationOptions, DurationStyle, format_duration};
pub use error::{FormatError, Result};
pub use list::{ListOptions, ListStyle, ListType, format_list};
pub use measurement::{Measurement, MeasurementOptions, Unit, UnitWidth, format_measurement};
pub use message::{MessageArg, format_message};
pub use number::{LocaleNumberSymbols, NumberNotation, NumberOptions, SignDisplay, format_number};
pub use options::FormatOption;
pub use percent::{PercentOptions, PercentScale, format_percent};
pub use phone::{PhoneFormat, PhoneNumber, PhoneOptions, format_phone};
pub use plural::{PluralCategory, format_ordinal, select_cardinal};
pub use relative::{Clock, MockClock, RelativeOptions, RelativeStyle, SystemClock, format_relative, next_update_boundary};
pub use text::{TextOptions, TextTransform, format_text};
pub use time::{PlainTime, TimeOptions, TimeStyle, format_time};
pub use timezone::ZoneOffset;

/// High-level explicit formatting API.
pub mod format {
    use super::*;

    pub fn number(value: impl Into<Decimal>, options: &NumberOptions, context: &FormatContext) -> Result<String> {
        format_number(value, options, context)
    }

    pub fn currency(value: impl Into<Decimal>, options: &CurrencyOptions, context: &FormatContext) -> Result<String> {
        format_currency(value, options, context)
    }

    pub fn percent(value: impl Into<Decimal>, options: &PercentOptions, context: &FormatContext) -> Result<String> {
        format_percent(value, options, context)
    }

    pub fn date(date: PlainDate, options: &DateOptions, context: &FormatContext) -> Result<String> {
        format_date(date, options, context)
    }

    pub fn time(time: PlainTime, options: &TimeOptions, context: &FormatContext) -> Result<String> {
        format_time(time, options, context)
    }

    pub fn datetime(datetime: PlainDateTime, options: &DateTimeOptions, context: &FormatContext) -> Result<String> {
        format_datetime(datetime, options, context)
    }

    pub fn instant(instant: Instant, options: &DateTimeOptions, context: &FormatContext) -> Result<String> {
        format_instant(instant, options, context)
    }

    pub fn relative(target: Instant, options: &RelativeOptions, context: &FormatContext) -> Result<String> {
        format_relative(target, options, context)
    }

    pub fn duration(seconds: i64, options: &DurationOptions, context: &FormatContext) -> Result<String> {
        format_duration(seconds, options, context)
    }

    pub fn measurement(measurement: Measurement, options: &MeasurementOptions, context: &FormatContext) -> Result<String> {
        format_measurement(measurement, options, context)
    }

    pub fn bytes(bytes: u64, options: &ByteOptions, context: &FormatContext) -> Result<String> {
        format_bytes(bytes, options, context)
    }

    pub fn data_rate(rate: f64, options: &DataRateOptions, context: &FormatContext) -> Result<String> {
        format_data_rate(rate, options, context)
    }

    pub fn text(text: &str, options: &TextOptions, context: &FormatContext) -> Result<String> {
        format_text(text, options, context)
    }

    pub fn list(items: &[impl AsRef<str>], options: &ListOptions, context: &FormatContext) -> Result<String> {
        format_list(items, options, context)
    }

    pub fn ordinal(number: i64, context: &FormatContext) -> Result<String> {
        format_ordinal(number, context)
    }

    pub fn message(template: &str, args: &[(&str, MessageArg)], context: &FormatContext) -> Result<String> {
        format_message(template, args, context)
    }

    pub fn phone(number: &PhoneNumber, options: &PhoneOptions, context: &FormatContext) -> Result<String> {
        format_phone(number, options, context)
    }

    pub fn address(address: &PostalAddress, options: &AddressOptions, context: &FormatContext) -> Result<String> {
        format_address(address, options, context)
    }

    /// Format a raw string value according to a FormatOption and context.
    /// Used by Omen Studio live previews and dynamic bindings.
    pub fn format_dynamic(input: &str, option: &FormatOption, context: &FormatContext) -> Result<String> {
        match option {
            FormatOption::Number(opts) => {
                let dec = parse::parse_number(input, opts, context)?;
                format_number(dec, opts, context)
            }
            FormatOption::Currency(opts) => {
                let dec = parse::parse_number(input, &NumberOptions::default(), context)?;
                format_currency(dec, opts, context)
            }
            FormatOption::Percent(opts) => {
                let dec = parse::parse_number(input, &NumberOptions::default(), context)?;
                format_percent(dec, opts, context)
            }
            FormatOption::Date(opts) => {
                let d = parse::parse_date(input, context)?;
                format_date(d, opts, context)
            }
            FormatOption::Time(opts) => {
                let parts: Vec<&str> = input.split(':').collect();
                let h: u8 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
                let m: u8 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
                let s: u8 = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
                let t = PlainTime::new(h, m, s, 0)?;
                format_time(t, opts, context)
            }
            FormatOption::DateTime(opts) => {
                if let Ok(sec) = input.parse::<i64>() {
                    format_instant(Instant::from_epoch_seconds(sec), opts, context)
                } else {
                    let (d_str, t_str) = if let Some(pos) = input.find('T').or_else(|| input.find(' ')) {
                        (&input[..pos], input[pos + 1..].trim_end_matches('Z'))
                    } else {
                        (input, "00:00:00")
                    };
                    let d = parse::parse_date(d_str, context).unwrap_or(PlainDate { year: 2026, month: 11, day: 23 });
                    let parts: Vec<&str> = t_str.split(':').collect();
                    let h: u8 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
                    let m: u8 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
                    let s: u8 = parts.get(2).and_then(|p| p.split('.').next()?.parse().ok()).unwrap_or(0);
                    let t = PlainTime { hour: h, minute: m, second: s, nanosecond: 0 };
                    format_datetime(PlainDateTime::new(d, t), opts, context)
                }
            }
            FormatOption::Relative(opts) => {
                let sec: i64 = input.parse().unwrap_or(0);
                format_relative(Instant::from_epoch_seconds(sec), opts, context)
            }
            FormatOption::Duration(opts) => {
                let sec: i64 = input.parse().unwrap_or(0);
                format_duration(sec, opts, context)
            }
            FormatOption::Measurement(opts) => {
                let m = parse::parse_measurement(input, context)?;
                format_measurement(m, opts, context)
            }
            FormatOption::Bytes(opts) => {
                let b: u64 = input.parse().unwrap_or(0);
                format_bytes(b, opts, context)
            }
            FormatOption::DataRate(opts) => {
                let r: f64 = input.parse().unwrap_or(0.0);
                format_data_rate(r, opts, context)
            }
            FormatOption::Text(opts) => format_text(input, opts, context),
            FormatOption::List(opts) => {
                let items: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
                format_list(&items, opts, context)
            }
            FormatOption::Phone(opts) => {
                let p = PhoneNumber::parse(input, opts.default_country_code)?;
                format_phone(&p, opts, context)
            }
            FormatOption::Address(opts) => {
                let addr = PostalAddress {
                    street: input.to_string(),
                    locality: "City".into(),
                    admin_area: None,
                    postal_code: "12345".into(),
                    country_code: "US".into(),
                    recipient: None,
                };
                format_address(&addr, opts, context)
            }
        }
    }
}
