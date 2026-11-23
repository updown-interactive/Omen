//! Time zone handling, conversions, and representation.
use serde::{Deserialize, Serialize};

/// Resolved time zone offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneOffset {
    seconds: i32,
}

impl ZoneOffset {
    pub const UTC: Self = Self { seconds: 0 };

    pub fn from_seconds(seconds: i32) -> Self {
        Self { seconds }
    }

    pub fn from_hours_minutes(hours: i32, minutes: i32) -> Self {
        let sign = if hours < 0 { -1 } else { 1 };
        let total = hours * 3600 + sign * (minutes.abs() * 60);
        Self { seconds: total }
    }

    pub fn total_seconds(self) -> i32 {
        self.seconds
    }

    pub fn format_offset(self) -> String {
        if self.seconds == 0 {
            "UTC".to_string()
        } else {
            let sign = if self.seconds >= 0 { '+' } else { '-' };
            let abs = self.seconds.abs();
            let hours = abs / 3600;
            let mins = (abs % 3600) / 60;
            format!("UTC{sign}{hours:02}:{mins:02}")
        }
    }
}

/// Convert UTC timestamp (seconds) into local components (year, month, day, hour, minute, second).
pub fn utc_to_local_components(utc_timestamp_sec: i64, offset_sec: i32) -> (i32, u8, u8, u8, u8, u8, u8) {
    let local_timestamp = utc_timestamp_sec + offset_sec as i64;
    // Days since Unix epoch (1970-01-01)
    let total_days = local_timestamp.div_euclid(86400);
    let day_seconds = local_timestamp.rem_euclid(86400) as u32;

    let hour = (day_seconds / 3600) as u8;
    let minute = ((day_seconds % 3600) / 60) as u8;
    let second = (day_seconds % 60) as u8;

    // Day of week: 1970-01-01 was Thursday (4)
    // 0 = Sunday, 1 = Monday, ..., 6 = Saturday
    let weekday = ((total_days + 4).rem_euclid(7)) as u8;

    // Civil date calculation from days since 1970-01-01
    let (year, month, day) = days_to_date(total_days);

    (year, month, day, hour, minute, second, weekday)
}

fn days_to_date(days_since_1970: i64) -> (i32, u8, u8) {
    // Gregorian calendar algorithm
    let z = days_since_1970 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1020 + doe / 1461 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u8, d as u8)
}

pub fn date_to_days(year: i32, month: u8, day: u8) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = if month <= 2 { month + 9 } else { month - 3 } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * m + 2) / 5 + day as i64 - 1;
    let doe = yoe as i64 * 365 + (yoe / 4) as i64 - (yoe / 100) as i64 + doy;
    era * 146097 + doe - 719468
}
