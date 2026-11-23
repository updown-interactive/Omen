//! Parsing API for converting localized user text into typed domain values.
use crate::context::FormatContext;
use crate::currency::{CurrencyOptions, Money, currency_symbol};
use crate::date::PlainDate;
use crate::decimal::Decimal;
use crate::error::{FormatError, Result};
use crate::measurement::{Measurement, Unit};
use crate::number::{LocaleNumberSymbols, NumberOptions};
use std::str::FromStr;

/// Parse a localized number string into a typed Decimal value.
pub fn parse_number(
    input: &str,
    _options: &NumberOptions,
    context: &FormatContext,
) -> Result<Decimal> {
    let s = input.trim();
    if s.is_empty() {
        return Err(FormatError::ParseError("empty number string".into()));
    }

    // Fast-path: standard invariant numeric format (e.g. from Rust literals, JSON, database)
    if let Ok(d) = Decimal::from_str(s) {
        return Ok(d);
    }

    let symbols = LocaleNumberSymbols::for_locale(context.locale.as_str());

    // Remove grouping separators and replace localized decimal separator with standard '.'
    let mut normalized = String::new();
    let mut chars = s.chars().peekable();

    let is_neg = s.starts_with('-');
    if is_neg || s.starts_with('+') {
        chars.next();
    }

    let dec_char = symbols.decimal.chars().next().unwrap_or('.');
    let group_char = symbols.grouping.chars().next().unwrap_or(',');

    for c in chars {
        if c == group_char || c == ' ' || c == '\u{00A0}' {
            continue;
        } else if c == dec_char || c == '.' || c == '٫' {
            normalized.push('.');
        } else if c.is_ascii_digit() {
            normalized.push(c);
        } else if ('\u{0660}'..='\u{0669}').contains(&c) {
            // Arabic-Indic digit
            let digit = ((c as u32) - 0x0660) as u8;
            normalized.push((b'0' + digit) as char);
        } else if ('\u{0966}'..='\u{096F}').contains(&c) {
            // Devanagari digit
            let digit = ((c as u32) - 0x0966) as u8;
            normalized.push((b'0' + digit) as char);
        } else {
            return Err(FormatError::ParseError(format!("unexpected character in number: '{c}'")));
        }
    }

    if is_neg {
        normalized.insert(0, '-');
    }

    Decimal::from_str(&normalized)
}

/// Parse a currency string into a typed Money value.
pub fn parse_currency(
    input: &str,
    options: &CurrencyOptions,
    context: &FormatContext,
) -> Result<Money> {
    let s = input.trim();
    let sym = currency_symbol(options.currency);
    let code = options.currency.as_str();

    // Strip symbol or code
    let stripped = s
        .trim_start_matches(&sym)
        .trim_start_matches(code)
        .trim_end_matches(&sym)
        .trim_end_matches(code)
        .trim();

    // Check for accounting parentheses: (100.00) -> -100.00
    let (is_accounting_neg, num_str) = if stripped.starts_with('(') && stripped.ends_with(')') {
        (true, &stripped[1..stripped.len() - 1])
    } else {
        (false, stripped)
    };

    let mut dec = parse_number(num_str, &NumberOptions::default(), context)?;
    if is_accounting_neg && !dec.is_negative() {
        dec = dec.negate();
    }

    Ok(Money::new(dec, options.currency))
}

/// Parse a date string in ISO or standard localized formats.
pub fn parse_date(
    input: &str,
    _context: &FormatContext,
) -> Result<PlainDate> {
    let s = input.trim();
    // Try ISO 8601 YYYY-MM-DD
    let parts: Vec<&str> = s.split(['-', '/', '.']).collect();
    if parts.len() != 3 {
        return Err(FormatError::ParseError(format!("cannot parse date '{input}'")));
    }

    let p0: i32 = parts[0].parse().map_err(|e| FormatError::ParseError(format!("{e}")))?;
    let p1: u32 = parts[1].parse().map_err(|e| FormatError::ParseError(format!("{e}")))?;
    let p2: i32 = parts[2].parse().map_err(|e| FormatError::ParseError(format!("{e}")))?;

    if p0 > 1000 {
        // YYYY-MM-DD
        PlainDate::new(p0, p1 as u8, p2 as u8)
    } else if p2 > 1000 {
        // DD/MM/YYYY or MM/DD/YYYY
        if p0 > 12 {
            // DD/MM/YYYY
            PlainDate::new(p2, p1 as u8, p0 as u8)
        } else {
            // Default MM/DD/YYYY
            PlainDate::new(p2, p0 as u8, p1 as u8)
        }
    } else {
        Err(FormatError::ParseError(format!("ambiguous 2-digit year in date '{input}'")))
    }
}

/// Parse a measurement string with quantity and unit.
pub fn parse_measurement(
    input: &str,
    context: &FormatContext,
) -> Result<Measurement> {
    let s = input.trim();
    let mut split_idx = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || c == '.' || c == ',' || c == '-' || c == '+' {
            split_idx = i + c.len_utf8();
        }
    }

    let num_part = s[..split_idx].trim();
    let unit_part = s[split_idx..].trim();

    let dec = parse_number(num_part, &NumberOptions::default(), context)?;
    let unit = match unit_part.to_lowercase().as_str() {
        "km" | "kilometer" | "kilometers" => Unit::Kilometer,
        "m" | "meter" | "meters" => Unit::Meter,
        "cm" | "centimeter" | "centimeters" => Unit::Centimeter,
        "mm" | "millimeter" | "millimeters" => Unit::Millimeter,
        "mi" | "mile" | "miles" => Unit::Mile,
        "kg" | "kilogram" | "kilograms" => Unit::Kilogram,
        "g" | "gram" | "grams" => Unit::Gram,
        "lb" | "lbs" | "pound" | "pounds" => Unit::Pound,
        "c" | "°c" | "celsius" => Unit::Celsius,
        "f" | "°f" | "fahrenheit" => Unit::Fahrenheit,
        _ => return Err(FormatError::ParseError(format!("unknown unit '{unit_part}'"))),
    };

    Ok(Measurement::new(dec.to_f64(), unit))
}
