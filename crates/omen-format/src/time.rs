//! Plain time representation and time formatting.
use crate::context::{FormatContext, HourCycle};
use crate::error::{FormatError, Result};
use serde::{Deserialize, Serialize};

/// Wall-clock time without an associated date or time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlainTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

impl PlainTime {
    pub fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Result<Self> {
        if hour > 23 {
            return Err(FormatError::Failed(format!("invalid hour: {hour}")));
        }
        if minute > 59 {
            return Err(FormatError::Failed(format!("invalid minute: {minute}")));
        }
        if second > 59 {
            return Err(FormatError::Failed(format!("invalid second: {second}")));
        }
        if nanosecond > 999_999_999 {
            return Err(FormatError::Failed(format!("invalid nanosecond: {nanosecond}")));
        }
        Ok(Self { hour, minute, second, nanosecond })
    }

    pub fn from_hms(hour: u8, minute: u8, second: u8) -> Result<Self> {
        Self::new(hour, minute, second, 0)
    }

    pub fn from_hm(hour: u8, minute: u8) -> Result<Self> {
        Self::new(hour, minute, 0, 0)
    }
}

/// Standard time formatting style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeStyle {
    #[default]
    Short,  // 08:30 or 8:30 AM
    Medium, // 08:30:45 or 8:30:45 AM
    Long,   // Includes time zone or seconds + milliseconds
}

/// Options configuring time formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeOptions {
    pub style: Option<TimeStyle>,
    pub hour_cycle: Option<HourCycle>,
    pub include_seconds: Option<bool>,
    pub include_fractional_seconds: bool,
    pub fractional_digits: usize,
}

impl TimeOptions {
    pub fn short() -> Self {
        Self { style: Some(TimeStyle::Short), ..Self::default() }
    }

    pub fn medium() -> Self {
        Self { style: Some(TimeStyle::Medium), ..Self::default() }
    }

    pub fn long() -> Self {
        Self { style: Some(TimeStyle::Long), ..Self::default() }
    }

    pub fn hour_cycle(mut self, cycle: HourCycle) -> Self {
        self.hour_cycle = Some(cycle);
        self
    }

    pub fn seconds(mut self, include: bool) -> Self {
        self.include_seconds = Some(include);
        self
    }

    pub fn fractional_seconds(mut self, digits: usize) -> Self {
        self.include_fractional_seconds = true;
        self.fractional_digits = digits;
        self
    }
}

/// Format a time with options and context.
pub fn format_time(time: PlainTime, options: &TimeOptions, context: &FormatContext) -> Result<String> {
    let lang = context.locale.language();
    let region = context.locale.region().unwrap_or("");

    // Determine hour cycle (12h vs 24h)
    let is_12h = match options.hour_cycle.unwrap_or(context.hour_cycle) {
        HourCycle::H11 | HourCycle::H12 => true,
        HourCycle::H23 | HourCycle::H24 => false,
        HourCycle::Auto => {
            // US, UK, IN typically use 12h in standard display; DE, FR, JP use 24h
            matches!(region, "US" | "IN") || (lang == "en" && !region.eq_ignore_ascii_case("GB"))
        }
    };

    let style = options.style.unwrap_or(TimeStyle::Short);
    let with_seconds = options.include_seconds.unwrap_or(style != TimeStyle::Short);

    let (display_hour, am_pm) = if is_12h {
        let h = time.hour;
        let is_pm = h >= 12;
        let h12 = match h % 12 {
            0 => 12,
            other => other,
        };
        let period = if is_pm {
            match lang {
                "de" => "nachm.",
                "fr" => "PM",
                "ja" => "午後",
                "ar" => "م",
                "hi" => "अपराह्न",
                _ => "PM",
            }
        } else {
            match lang {
                "de" => "vorm.",
                "fr" => "AM",
                "ja" => "午前",
                "ar" => "ص",
                "hi" => "पूर्वाह्न",
                _ => "AM",
            }
        };
        (h12, Some(period))
    } else {
        (time.hour, None)
    };

    let base_time = if with_seconds {
        format!("{:02}:{:02}:{:02}", display_hour, time.minute, time.second)
    } else {
        format!("{:02}:{:02}", display_hour, time.minute)
    };

    let time_with_frac = if options.include_fractional_seconds && options.fractional_digits > 0 {
        let factor = 10u32.pow((9 - options.fractional_digits.min(9)) as u32);
        let frac = time.nanosecond / factor;
        format!("{base_time}.{:0width$}", frac, width = options.fractional_digits)
    } else {
        base_time
    };

    let output = if let Some(period) = am_pm {
        if lang == "ja" {
            format!("{period} {time_with_frac}")
        } else {
            format!("{time_with_frac} {period}")
        }
    } else {
        time_with_frac
    };

    Ok(output)
}
