//! Relative date and time formatting with injected clocks and update boundary calculation.
use crate::context::FormatContext;
use crate::datetime::Instant;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Injected clock for deterministic testing of relative time.
pub trait Clock: std::fmt::Debug + Send + Sync {
    fn now(&self) -> Instant;
}

/// Standard system wall clock.
#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Instant {
            seconds: now.as_secs() as i64,
            nanoseconds: now.subsec_nanos(),
        }
    }
}

/// Mock clock for deterministic tests.
#[derive(Debug)]
pub struct MockClock {
    current: std::sync::atomic::AtomicI64,
}

impl MockClock {
    pub fn at(seconds: i64) -> Self {
        Self {
            current: std::sync::atomic::AtomicI64::new(seconds),
        }
    }

    pub fn advance(&self, seconds: i64) {
        self.current.fetch_add(seconds, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        Instant {
            seconds: self.current.load(std::sync::atomic::Ordering::SeqCst),
            nanoseconds: 0,
        }
    }
}

/// Relative time style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelativeStyle {
    #[default]
    Natural, // "yesterday", "just now", "5 minutes ago"
    Numeric, // "1 day ago", "0 seconds ago", "5 minutes ago"
}

/// Options configuring relative date and time formatting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RelativeOptions {
    pub style: RelativeStyle,
    pub max_relative_days: Option<i64>,
    #[serde(skip)]
    pub clock: Option<Arc<dyn Clock>>,
}

impl PartialEq for RelativeOptions {
    fn eq(&self, other: &Self) -> bool {
        self.style == other.style && self.max_relative_days == other.max_relative_days
    }
}

impl Eq for RelativeOptions {}

impl RelativeOptions {
    pub fn natural() -> Self {
        Self { style: RelativeStyle::Natural, ..Self::default() }
    }

    pub fn numeric() -> Self {
        Self { style: RelativeStyle::Numeric, ..Self::default() }
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }
}

/// Calculate the duration until the displayed relative text will change.
pub fn next_update_boundary(target: Instant, now: Instant) -> Duration {
    let diff = (now.seconds - target.seconds).abs();
    let sec_until = if diff < 45 {
        45 - diff
    } else if diff < 90 {
        90 - diff
    } else if diff < 3600 {
        60 - (diff % 60)
    } else if diff < 86400 {
        3600 - (diff % 3600)
    } else {
        86400 - (diff % 86400)
    };
    Duration::from_secs(sec_until.max(1) as u64)
}

/// Format relative time between target and current clock time.
pub fn format_relative(
    target: Instant,
    options: &RelativeOptions,
    context: &FormatContext,
) -> Result<String> {
    let sys_clock = SystemClock;
    let now = match &options.clock {
        Some(c) => c.now(),
        None => sys_clock.now(),
    };

    let diff_seconds = now.seconds - target.seconds;
    let is_past = diff_seconds >= 0;
    let abs_seconds = diff_seconds.abs();
    let lang = context.locale.language();

    // Check maximum relative range fallback
    if let Some(max_days) = options.max_relative_days {
        if abs_seconds > max_days * 86400 {
            let (y, m, d, _, _, _, _) = crate::timezone::utc_to_local_components(target.seconds, 0);
            let date = crate::date::PlainDate { year: y, month: m, day: d };
            return crate::date::format_date(date, &crate::date::DateOptions::medium(), context);
        }
    }

    let output = if abs_seconds < 45 && options.style == RelativeStyle::Natural {
        match lang {
            "de" => "gerade eben",
            "fr" => "à l’instant",
            "es" => "ahora mismo",
            "ja" => "たった今",
            "ar" => "الآن",
            "hi" => "अभी-अभी",
            _ => "just now",
        }
        .to_string()
    } else if abs_seconds < 90 {
        // 1 minute
        format_unit(1, "minute", is_past, lang)
    } else if abs_seconds < 3600 {
        let minutes = (abs_seconds + 30) / 60;
        format_unit(minutes, "minute", is_past, lang)
    } else if abs_seconds < 7200 {
        // 1 hour
        format_unit(1, "hour", is_past, lang)
    } else if abs_seconds < 86400 {
        let hours = (abs_seconds + 1800) / 3600;
        format_unit(hours, "hour", is_past, lang)
    } else if abs_seconds < 172800 {
        // 1 day
        if options.style == RelativeStyle::Natural {
            if is_past {
                match lang {
                    "de" => "gestern",
                    "fr" => "hier",
                    "es" => "ayer",
                    "ja" => "昨日",
                    "ar" => "أمس",
                    "hi" => "कल",
                    _ => "yesterday",
                }
                .to_string()
            } else {
                match lang {
                    "de" => "morgen",
                    "fr" => "demain",
                    "es" => "mañana",
                    "ja" => "明日",
                    "ar" => "غداً",
                    "hi" => "कल",
                    _ => "tomorrow",
                }
                .to_string()
            }
        } else {
            format_unit(1, "day", is_past, lang)
        }
    } else if abs_seconds < 604800 {
        let days = (abs_seconds + 43200) / 86400;
        format_unit(days, "day", is_past, lang)
    } else if abs_seconds < 2592000 {
        let weeks = (abs_seconds + 302400) / 604800;
        format_unit(weeks, "week", is_past, lang)
    } else if abs_seconds < 31536000 {
        let months = (abs_seconds + 1296000) / 2592000;
        format_unit(months, "month", is_past, lang)
    } else {
        let years = (abs_seconds + 15768000) / 31536000;
        format_unit(years, "year", is_past, lang)
    };

    Ok(output)
}

fn format_unit(count: i64, unit: &str, is_past: bool, lang: &str) -> String {
    let is_singular = count == 1;
    match lang {
        "de" => {
            let u = match unit {
                "minute" => if is_singular { "Minute" } else { "Minuten" },
                "hour" => if is_singular { "Stunde" } else { "Stunden" },
                "day" => if is_singular { "Tag" } else { "Tagen" },
                "week" => if is_singular { "Woche" } else { "Wochen" },
                "month" => if is_singular { "Monat" } else { "Monaten" },
                "year" => if is_singular { "Jahr" } else { "Jahren" },
                _ => unit,
            };
            if is_past {
                format!("vor {count} {u}")
            } else {
                format!("in {count} {u}")
            }
        }
        "fr" => {
            let u = match unit {
                "minute" => if is_singular { "minute" } else { "minutes" },
                "hour" => if is_singular { "heure" } else { "heures" },
                "day" => if is_singular { "jour" } else { "jours" },
                "week" => if is_singular { "semaine" } else { "semaines" },
                "month" => "mois",
                "year" => if is_singular { "an" } else { "ans" },
                _ => unit,
            };
            if is_past {
                format!("il y a {count} {u}")
            } else {
                format!("dans {count} {u}")
            }
        }
        "ja" => {
            let u = match unit {
                "minute" => "分",
                "hour" => "時間",
                "day" => "日",
                "week" => "週間",
                "month" => "ヶ月",
                "year" => "年",
                _ => unit,
            };
            if is_past {
                format!("{count}{u}前")
            } else {
                format!("{count}{u}後")
            }
        }
        "hi" => {
            let u = match unit {
                "minute" => "मिनट",
                "hour" => "घंटे",
                "day" => "दिन",
                "week" => "सप्ताह",
                "month" => "महीने",
                "year" => "साल",
                _ => unit,
            };
            if is_past {
                format!("{count} {u} पहले")
            } else {
                format!("{count} {u} में")
            }
        }
        "ar" => {
            let u = match unit {
                "minute" => if is_singular { "دقيقة" } else { "دقائق" },
                "hour" => if is_singular { "ساعة" } else { "ساعات" },
                "day" => if is_singular { "يوم" } else { "أيام" },
                "week" => if is_singular { "أسبوع" } else { "أسابيع" },
                "month" => if is_singular { "شهر" } else { "أشهر" },
                "year" => if is_singular { "سنة" } else { "سنوات" },
                _ => unit,
            };
            if is_past {
                format!("منذ {count} {u}")
            } else {
                format!("خلال {count} {u}")
            }
        }
        _ => {
            let u = match unit {
                "minute" => if is_singular { "minute" } else { "minutes" },
                "hour" => if is_singular { "hour" } else { "hours" },
                "day" => if is_singular { "day" } else { "days" },
                "week" => if is_singular { "week" } else { "weeks" },
                "month" => if is_singular { "month" } else { "months" },
                "year" => if is_singular { "year" } else { "years" },
                _ => unit,
            };
            if is_past {
                format!("{count} {u} ago")
            } else {
                format!("in {count} {u}")
            }
        }
    }
}
