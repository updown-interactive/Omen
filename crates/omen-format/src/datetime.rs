//! Combined date and time representations and formatting with time zones.
use crate::context::{FormatContext, TimeZoneId};
use crate::date::{DateOptions, PlainDate, format_date};
use crate::error::Result;
use crate::time::{PlainTime, TimeOptions, format_time};
use crate::timezone::{ZoneOffset, date_to_days, utc_to_local_components};
use serde::{Deserialize, Serialize};

/// Combined local date and time without an explicit time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PlainDateTime {
    pub date: PlainDate,
    pub time: PlainTime,
}

impl PlainDateTime {
    pub fn new(date: PlainDate, time: PlainTime) -> Self {
        Self { date, time }
    }
}

/// An absolute instant in time (seconds + nanoseconds since Unix epoch).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Instant {
    pub seconds: i64,
    pub nanoseconds: u32,
}

impl Instant {
    pub const EPOCH: Self = Self { seconds: 0, nanoseconds: 0 };

    pub fn from_epoch_seconds(seconds: i64) -> Self {
        Self { seconds, nanoseconds: 0 }
    }

    pub fn from_epoch_millis(millis: i64) -> Self {
        let seconds = millis.div_euclid(1000);
        let nanos = (millis.rem_euclid(1000) * 1_000_000) as u32;
        Self { seconds, nanoseconds: nanos }
    }

    pub fn to_zoned(self, time_zone: &TimeZoneId) -> ZonedDateTime {
        let offset_sec = time_zone.fixed_offset_seconds();
        let (y, m, d, h, min, s, _) = utc_to_local_components(self.seconds, offset_sec);
        let date = PlainDate { year: y, month: m, day: d };
        let time = PlainTime { hour: h, minute: min, second: s, nanosecond: self.nanoseconds };
        ZonedDateTime {
            datetime: PlainDateTime { date, time },
            time_zone: time_zone.clone(),
            offset: ZoneOffset::from_seconds(offset_sec),
        }
    }
}

/// A zoned date and time anchored to an explicit time zone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZonedDateTime {
    pub datetime: PlainDateTime,
    pub time_zone: TimeZoneId,
    pub offset: ZoneOffset,
}

impl ZonedDateTime {
    pub fn to_instant(&self) -> Instant {
        let days = date_to_days(self.datetime.date.year, self.datetime.date.month, self.datetime.date.day);
        let day_seconds = (self.datetime.time.hour as i64) * 3600
            + (self.datetime.time.minute as i64) * 60
            + (self.datetime.time.second as i64);
        let local_timestamp = days * 86400 + day_seconds;
        let utc_timestamp = local_timestamp - (self.offset.total_seconds() as i64);
        Instant {
            seconds: utc_timestamp,
            nanoseconds: self.datetime.time.nanosecond,
        }
    }
}

/// Options configuring combined date and time formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateTimeOptions {
    pub date: DateOptions,
    pub time: TimeOptions,
    pub time_zone: Option<TimeZoneId>,
    pub include_time_zone_name: bool,
}

impl DateTimeOptions {
    pub fn short() -> Self {
        Self {
            date: DateOptions::short(),
            time: TimeOptions::short(),
            ..Self::default()
        }
    }

    pub fn medium() -> Self {
        Self {
            date: DateOptions::medium(),
            time: TimeOptions::medium(),
            ..Self::default()
        }
    }

    pub fn long() -> Self {
        Self {
            date: DateOptions::long(),
            time: TimeOptions::long(),
            include_time_zone_name: true,
            ..Self::default()
        }
    }

    pub fn full() -> Self {
        Self {
            date: DateOptions::full(),
            time: TimeOptions::long(),
            include_time_zone_name: true,
            ..Self::default()
        }
    }

    pub fn time_zone(mut self, time_zone: impl Into<TimeZoneId>) -> Self {
        self.time_zone = Some(time_zone.into());
        self
    }
}

/// Format a date-time (PlainDateTime, Instant, or ZonedDateTime).
pub fn format_datetime(
    datetime: PlainDateTime,
    options: &DateTimeOptions,
    context: &FormatContext,
) -> Result<String> {
    let date_str = format_date(datetime.date, &options.date, context)?;
    let time_str = format_time(datetime.time, &options.time, context)?;

    let mut output = format!("{date_str}, {time_str}");

    if options.include_time_zone_name {
        let tz = options.time_zone.as_ref().unwrap_or(&context.time_zone);
        output = format!("{output} {tz}");
    }

    Ok(output)
}

/// Format an Instant converting to the active time zone.
pub fn format_instant(
    instant: Instant,
    options: &DateTimeOptions,
    context: &FormatContext,
) -> Result<String> {
    let target_tz = options.time_zone.as_ref().unwrap_or(&context.time_zone);
    let zoned = instant.to_zoned(target_tz);
    format_datetime(zoned.datetime, options, context)
}
