//! Plain date representation and locale-aware date formatting.
use crate::context::FormatContext;
use crate::error::{FormatError, Result};
use crate::timezone::date_to_days;
use serde::{Deserialize, Serialize};

/// Calendar date without an associated time or time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlainDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl PlainDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self> {
        if !(1..=12).contains(&month) {
            return Err(FormatError::Failed(format!("invalid month: {month}")));
        }
        let max_days = days_in_month(year, month);
        if !(1..=max_days).contains(&day) {
            return Err(FormatError::Failed(format!("invalid day {day} for month {month} in year {year}")));
        }
        Ok(Self { year, month, day })
    }

    /// Weekday: 0 = Sunday, 1 = Monday, ..., 6 = Saturday
    pub fn weekday(&self) -> u8 {
        let days = date_to_days(self.year, self.month, self.day);
        ((days + 4).rem_euclid(7)) as u8
    }

    pub fn is_leap_year(&self) -> bool {
        is_leap_year(self.year)
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 30,
    }
}

/// Standard date formatting style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DateStyle {
    #[default]
    Medium, // e.g. Nov 23, 2026
    Short,  // e.g. 11/23/26 or 23/11/2026
    Long,   // e.g. November 23, 2026
    Full,   // e.g. Monday, November 23, 2026
}

/// Options configuring date formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateOptions {
    pub style: Option<DateStyle>,
    pub pattern: Option<String>,
    pub year_only: bool,
    pub month_and_year: bool,
}

impl DateOptions {
    pub fn short() -> Self {
        Self { style: Some(DateStyle::Short), ..Self::default() }
    }

    pub fn medium() -> Self {
        Self { style: Some(DateStyle::Medium), ..Self::default() }
    }

    pub fn long() -> Self {
        Self { style: Some(DateStyle::Long), ..Self::default() }
    }

    pub fn full() -> Self {
        Self { style: Some(DateStyle::Full), ..Self::default() }
    }

    pub fn pattern(pattern: impl Into<String>) -> Self {
        Self { pattern: Some(pattern.into()), ..Self::default() }
    }

    pub fn year_only() -> Self {
        Self { year_only: true, ..Self::default() }
    }

    pub fn month_and_year() -> Self {
        Self { month_and_year: true, ..Self::default() }
    }
}

/// Format a date with options and context.
pub fn format_date(date: PlainDate, options: &DateOptions, context: &FormatContext) -> Result<String> {
    let lang = context.locale.language();
    let region = context.locale.region().unwrap_or("");

    if options.year_only {
        return Ok(date.year.to_string());
    }

    if let Some(pattern) = &options.pattern {
        return format_with_pattern(date, pattern, lang);
    }

    if options.month_and_year {
        let month_name = localized_month_name(lang, date.month, false);
        return Ok(match lang {
            "ja" => format!("{}年{}月", date.year, date.month),
            _ => format!("{month_name} {}", date.year),
        });
    }

    let style = options.style.unwrap_or(DateStyle::Medium);
    let output = match style {
        DateStyle::Short => match (lang, region) {
            ("en", r) if r.eq_ignore_ascii_case("US") => {
                format!("{:02}/{:02}/{:02}", date.month, date.day, (date.year % 100).abs())
            }
            ("en", r) if r.eq_ignore_ascii_case("GB") || r.eq_ignore_ascii_case("IN") => {
                format!("{:02}/{:02}/{}", date.day, date.month, date.year)
            }
            ("de", _) => format!("{:02}.{:02}.{}", date.day, date.month, date.year),
            ("fr", _) | ("es", _) | ("it", _) => {
                format!("{:02}/{:02}/{}", date.day, date.month, date.year)
            }
            ("ja", _) => format!("{}/{:02}/{:02}", date.year, date.month, date.day),
            _ => format!("{:02}/{:02}/{}", date.day, date.month, date.year),
        },
        DateStyle::Medium => {
            let month_abbr = localized_month_name(lang, date.month, true);
            match (lang, region) {
                ("en", r) if r.eq_ignore_ascii_case("US") => {
                    format!("{month_abbr} {}, {}", date.day, date.year)
                }
                ("en", _) => format!("{} {month_abbr} {}", date.day, date.year),
                ("de", _) => format!("{}. {month_abbr} {}", date.day, date.year),
                ("fr", _) | ("es", _) | ("it", _) => {
                    format!("{} {month_abbr} {}", date.day, date.year)
                }
                ("ja", _) => format!("{}年{}月{}日", date.year, date.month, date.day),
                ("hi", _) => format!("{} {month_abbr} {}", date.day, date.year),
                ("ar", _) => format!("{} {month_abbr} {}", date.day, date.year),
                _ => format!("{month_abbr} {}, {}", date.day, date.year),
            }
        }
        DateStyle::Long => {
            let month_full = localized_month_name(lang, date.month, false);
            match (lang, region) {
                ("en", r) if r.eq_ignore_ascii_case("US") => {
                    format!("{month_full} {}, {}", date.day, date.year)
                }
                ("en", _) => format!("{} {month_full} {}", date.day, date.year),
                ("de", _) => format!("{}. {month_full} {}", date.day, date.year),
                ("fr", _) => format!("{} {month_full} {}", date.day, date.year),
                ("ja", _) => format!("{}年{}月{}日", date.year, date.month, date.day),
                _ => format!("{} {month_full} {}", date.day, date.year),
            }
        }
        DateStyle::Full => {
            let weekday = localized_weekday_name(lang, date.weekday(), false);
            let month_full = localized_month_name(lang, date.month, false);
            match (lang, region) {
                ("en", r) if r.eq_ignore_ascii_case("US") => {
                    format!("{weekday}, {month_full} {}, {}", date.day, date.year)
                }
                ("en", _) => format!("{weekday}, {} {month_full} {}", date.day, date.year),
                ("de", _) => format!("{weekday}, {}. {month_full} {}", date.day, date.year),
                ("fr", _) => format!("{weekday} {} {month_full} {}", date.day, date.year),
                ("ja", _) => format!("{}年{}月{}日 {}", date.year, date.month, date.day, weekday),
                _ => format!("{weekday}, {} {month_full} {}", date.day, date.year),
            }
        }
    };

    Ok(output)
}

fn format_with_pattern(date: PlainDate, pattern: &str, lang: &str) -> Result<String> {
    let mut output = pattern.to_string();
    let month_abbr = localized_month_name(lang, date.month, true);
    let month_full = localized_month_name(lang, date.month, false);
    let weekday_abbr = localized_weekday_name(lang, date.weekday(), true);
    let weekday_full = localized_weekday_name(lang, date.weekday(), false);

    output = output.replace("yyyy", &format!("{:04}", date.year));
    output = output.replace("yy", &format!("{:02}", (date.year % 100).abs()));
    output = output.replace("MMMM", month_full);
    output = output.replace("MMM", month_abbr);
    output = output.replace("MM", &format!("{:02}", date.month));
    output = output.replace("M", &date.month.to_string());
    output = output.replace("dd", &format!("{:02}", date.day));
    output = output.replace("d", &date.day.to_string());
    output = output.replace("EEEE", weekday_full);
    output = output.replace("EEE", weekday_abbr);

    Ok(output)
}

pub fn localized_month_name(lang: &str, month: u8, short: bool) -> &'static str {
    match lang {
        "de" => match (month, short) {
            (1, false) => "Januar", (1, true) => "Jan",
            (2, false) => "Februar", (2, true) => "Feb",
            (3, false) => "März", (3, true) => "Mär",
            (4, false) => "April", (4, true) => "Apr",
            (5, false) => "Mai", (5, true) => "Mai",
            (6, false) => "Juni", (6, true) => "Jun",
            (7, false) => "Juli", (7, true) => "Jul",
            (8, false) => "August", (8, true) => "Aug",
            (9, false) => "September", (9, true) => "Sep",
            (10, false) => "Oktober", (10, true) => "Okt",
            (11, false) => "November", (11, true) => "Nov",
            (12, false) => "Dezember", (12, true) => "Dez",
            _ => "",
        },
        "fr" => match (month, short) {
            (1, false) => "janvier", (1, true) => "janv.",
            (2, false) => "février", (2, true) => "févr.",
            (3, false) => "mars", (3, true) => "mars",
            (4, false) => "avril", (4, true) => "avr.",
            (5, false) => "mai", (5, true) => "mai",
            (6, false) => "juin", (6, true) => "juin",
            (7, false) => "juillet", (7, true) => "juil.",
            (8, false) => "août", (8, true) => "août",
            (9, false) => "septembre", (9, true) => "sept.",
            (10, false) => "octobre", (10, true) => "oct.",
            (11, false) => "novembre", (11, true) => "nov.",
            (12, false) => "décembre", (12, true) => "déc.",
            _ => "",
        },
        "ja" => match month {
            1 => "1月", 2 => "2月", 3 => "3月",
            4 => "4月", 5 => "5月", 6 => "6月",
            7 => "7月", 8 => "8月", 9 => "9月",
            10 => "10月", 11 => "11月", 12 => "12月",
            _ => "",
        },
        "hi" => match (month, short) {
            (1, false) => "जनवरी", (1, true) => "जन",
            (2, false) => "फ़रवरी", (2, true) => "फ़र",
            (3, false) => "मार्च", (3, true) => "मार्च",
            (4, false) => "अप्रैल", (4, true) => "अप्रै",
            (5, false) => "मई", (5, true) => "मई",
            (6, false) => "जून", (6, true) => "जून",
            (7, false) => "जुलाई", (7, true) => "जुला",
            (8, false) => "अगस्त", (8, true) => "अग",
            (9, false) => "सितंबर", (9, true) => "सितं",
            (10, false) => "अक्टूबर", (10, true) => "अक्टू",
            (11, false) => "नवंबर", (11, true) => "नवं",
            (12, false) => "दिसंबर", (12, true) => "दिसं",
            _ => "",
        },
        "ar" => match (month, short) {
            (1, false) => "يناير", (1, true) => "يناير",
            (2, false) => "فبراير", (2, true) => "فبراير",
            (3, false) => "مارس", (3, true) => "مارس",
            (4, false) => "أبريل", (4, true) => "أبريل",
            (5, false) => "مايو", (5, true) => "مايو",
            (6, false) => "يونيو", (6, true) => "يونيو",
            (7, false) => "يوليو", (7, true) => "يوليو",
            (8, false) => "أغسطس", (8, true) => "أغسطس",
            (9, false) => "سبتمبر", (9, true) => "سبتمبر",
            (10, false) => "أكتوبر", (10, true) => "أكتوبر",
            (11, false) => "نوفمبر", (11, true) => "نوفمبر",
            (12, false) => "ديسمبر", (12, true) => "ديسمبر",
            _ => "",
        },
        _ => match (month, short) {
            (1, false) => "January", (1, true) => "Jan",
            (2, false) => "February", (2, true) => "Feb",
            (3, false) => "March", (3, true) => "Mar",
            (4, false) => "April", (4, true) => "Apr",
            (5, false) => "May", (5, true) => "May",
            (6, false) => "June", (6, true) => "Jun",
            (7, false) => "July", (7, true) => "Jul",
            (8, false) => "August", (8, true) => "Aug",
            (9, false) => "September", (9, true) => "Sep",
            (10, false) => "October", (10, true) => "Oct",
            (11, false) => "November", (11, true) => "Nov",
            (12, false) => "December", (12, true) => "Dec",
            _ => "",
        },
    }
}

pub fn localized_weekday_name(lang: &str, weekday: u8, short: bool) -> &'static str {
    match lang {
        "de" => match (weekday, short) {
            (0, false) => "Sonntag", (0, true) => "So",
            (1, false) => "Montag", (1, true) => "Mo",
            (2, false) => "Dienstag", (2, true) => "Di",
            (3, false) => "Mittwoch", (3, true) => "Mi",
            (4, false) => "Donnerstag", (4, true) => "Do",
            (5, false) => "Freitag", (5, true) => "Fr",
            (6, false) => "Samstag", (6, true) => "Sa",
            _ => "",
        },
        "fr" => match (weekday, short) {
            (0, false) => "dimanche", (0, true) => "dim.",
            (1, false) => "lundi", (1, true) => "lun.",
            (2, false) => "mardi", (2, true) => "mar.",
            (3, false) => "mercredi", (3, true) => "mer.",
            (4, false) => "jeudi", (4, true) => "jeu.",
            (5, false) => "vendredi", (5, true) => "ven.",
            (6, false) => "samedi", (6, true) => "sam.",
            _ => "",
        },
        "ja" => match (weekday, short) {
            (0, false) => "日曜日", (0, true) => "日",
            (1, false) => "月曜日", (1, true) => "月",
            (2, false) => "火曜日", (2, true) => "火",
            (3, false) => "水曜日", (3, true) => "水",
            (4, false) => "木曜日", (4, true) => "木",
            (5, false) => "金曜日", (5, true) => "金",
            (6, false) => "土曜日", (6, true) => "土",
            _ => "",
        },
        "hi" => match (weekday, short) {
            (0, false) => "रविवार", (0, true) => "रवि",
            (1, false) => "सोमवार", (1, true) => "सोम",
            (2, false) => "मंगलवार", (2, true) => "मंगल",
            (3, false) => "बुधवार", (3, true) => "बुध",
            (4, false) => "गुरुवार", (4, true) => "गुरु",
            (5, false) => "शुक्रवार", (5, true) => "शुक्र",
            (6, false) => "शनिवार", (6, true) => "शनि",
            _ => "",
        },
        "ar" => match (weekday, short) {
            (0, false) => "الأحد", (0, true) => "أحد",
            (1, false) => "الإثنين", (1, true) => "إثنين",
            (2, false) => "الثلاثاء", (2, true) => "ثلاثاء",
            (3, false) => "الأربعاء", (3, true) => "أربعاء",
            (4, false) => "الخميس", (4, true) => "خميس",
            (5, false) => "الجمعة", (5, true) => "جمعة",
            (6, false) => "السبت", (6, true) => "سبت",
            _ => "",
        },
        _ => match (weekday, short) {
            (0, false) => "Sunday", (0, true) => "Sun",
            (1, false) => "Monday", (1, true) => "Mon",
            (2, false) => "Tuesday", (2, true) => "Tue",
            (3, false) => "Wednesday", (3, true) => "Wed",
            (4, false) => "Thursday", (4, true) => "Thu",
            (5, false) => "Friday", (5, true) => "Fri",
            (6, false) => "Saturday", (6, true) => "Sat",
            _ => "",
        },
    }
}
