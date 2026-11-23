//! Duration formatting for elapsed times, clock displays, and human-readable time spans.
use crate::context::FormatContext;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Duration display style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DurationStyle {
    #[default]
    Human, // e.g. "1 hour, 32 minutes"
    Clock, // e.g. "01:32:08"
    Short, // e.g. "1h 32m"
    Narrow, // e.g. "1h32m"
}

/// Options configuring duration formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationOptions {
    pub style: DurationStyle,
    pub max_units: Option<usize>,
    pub include_seconds: Option<bool>,
    pub include_zero_units: bool,
}

impl DurationOptions {
    pub fn human() -> Self {
        Self { style: DurationStyle::Human, ..Self::default() }
    }

    pub fn clock() -> Self {
        Self { style: DurationStyle::Clock, ..Self::default() }
    }

    pub fn short() -> Self {
        Self { style: DurationStyle::Short, ..Self::default() }
    }

    pub fn narrow() -> Self {
        Self { style: DurationStyle::Narrow, ..Self::default() }
    }

    pub fn max_units(mut self, max: usize) -> Self {
        self.max_units = Some(max);
        self
    }
}

/// Format a duration in seconds.
pub fn format_duration(
    seconds: i64,
    options: &DurationOptions,
    context: &FormatContext,
) -> Result<String> {
    let is_neg = seconds < 0;
    let abs_sec = seconds.abs();
    let lang = context.locale.language();

    if options.style == DurationStyle::Clock {
        let hours = abs_sec / 3600;
        let mins = (abs_sec % 3600) / 60;
        let secs = abs_sec % 60;
        let sign = if is_neg { "-" } else { "" };
        let clock = if hours > 0 {
            format!("{sign}{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{sign}{:02}:{:02}", mins, secs)
        };
        return Ok(clock);
    }

    let days = abs_sec / 86400;
    let hours = (abs_sec % 86400) / 3600;
    let mins = (abs_sec % 3600) / 60;
    let secs = abs_sec % 60;

    let mut parts = Vec::new();

    if days > 0 {
        parts.push((days, "day"));
    }
    if hours > 0 || (options.include_zero_units && !parts.is_empty()) {
        parts.push((hours, "hour"));
    }
    if mins > 0 || (options.include_zero_units && !parts.is_empty()) {
        parts.push((mins, "minute"));
    }
    if secs > 0 || parts.is_empty() {
        parts.push((secs, "second"));
    }

    if let Some(max) = options.max_units {
        parts.truncate(max);
    }

    let formatted_parts: Vec<String> = parts
        .into_iter()
        .map(|(count, unit)| {
            match options.style {
                DurationStyle::Short => match unit {
                    "day" => format!("{count}d"),
                    "hour" => format!("{count}h"),
                    "minute" => format!("{count}m"),
                    "second" => format!("{count}s"),
                    _ => format!("{count}"),
                },
                DurationStyle::Narrow => match unit {
                    "day" => format!("{count}d"),
                    "hour" => format!("{count}h"),
                    "minute" => format!("{count}m"),
                    "second" => format!("{count}s"),
                    _ => format!("{count}"),
                },
                _ => {
                    let is_singular = count == 1;
                    match lang {
                        "de" => match unit {
                            "day" => if is_singular { format!("{count} Tag") } else { format!("{count} Tage") },
                            "hour" => if is_singular { format!("{count} Stunde") } else { format!("{count} Stunden") },
                            "minute" => if is_singular { format!("{count} Minute") } else { format!("{count} Minuten") },
                            "second" => if is_singular { format!("{count} Sekunde") } else { format!("{count} Sekunden") },
                            _ => format!("{count} {unit}"),
                        },
                        "fr" => match unit {
                            "day" => if is_singular { format!("{count} jour") } else { format!("{count} jours") },
                            "hour" => if is_singular { format!("{count} heure") } else { format!("{count} heures") },
                            "minute" => if is_singular { format!("{count} minute") } else { format!("{count} minutes") },
                            "second" => if is_singular { format!("{count} seconde") } else { format!("{count} secondes") },
                            _ => format!("{count} {unit}"),
                        },
                        "ja" => match unit {
                            "day" => format!("{count}日"),
                            "hour" => format!("{count}時間"),
                            "minute" => format!("{count}分"),
                            "second" => format!("{count}秒"),
                            _ => format!("{count}{unit}"),
                        },
                        _ => match unit {
                            "day" => if is_singular { format!("{count} day") } else { format!("{count} days") },
                            "hour" => if is_singular { format!("{count} hour") } else { format!("{count} hours") },
                            "minute" => if is_singular { format!("{count} minute") } else { format!("{count} minutes") },
                            "second" => if is_singular { format!("{count} second") } else { format!("{count} seconds") },
                            _ => format!("{count} {unit}"),
                        },
                    }
                }
            }
        })
        .collect();

    let sep = match options.style {
        DurationStyle::Short => " ",
        DurationStyle::Narrow => "",
        _ => ", ",
    };

    let joined = formatted_parts.join(sep);
    let output = if is_neg { format!("-{joined}") } else { joined };

    Ok(output)
}
