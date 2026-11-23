//! Developer-defined custom formatters and machine-readable metadata for AI agents and Studio.
use crate::context::FormatContext;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Metadata describing a formatter's capabilities, input types, and configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatterDescriptor {
    pub name: &'static str,
    pub category: &'static str,
    pub input_type: &'static str,
    pub description: &'static str,
    pub example_input: &'static str,
    pub example_output: &'static str,
    pub options: Vec<&'static str>,
}

/// Core trait implemented by any typed formatter.
pub trait Formatter<T>: Send + Sync {
    fn format(&self, value: &T, context: &FormatContext) -> Result<String>;
    fn descriptor(&self) -> FormatterDescriptor;
}

/// Registry of standard formatters exposed to AI agents and Studio Inspector.
pub fn standard_formatter_descriptors() -> Vec<FormatterDescriptor> {
    vec![
        FormatterDescriptor {
            name: "number",
            category: "Number",
            input_type: "Number | Decimal | Float | Integer",
            description: "Locale-aware decimal number formatting with grouping and precision control",
            example_input: "1234567.89",
            example_output: "1,234,567.89",
            options: vec!["minimum_fraction_digits", "maximum_fraction_digits", "use_grouping", "compact", "scientific"],
        },
        FormatterDescriptor {
            name: "currency",
            category: "Currency",
            input_type: "Number | Decimal | Float | Money",
            description: "ISO 4217 currency formatting with localized symbols, placement, and accounting notation",
            example_input: "1234.50, USD",
            example_output: "$1,234.50",
            options: vec!["currency", "display (symbol/code/name)", "accounting", "fraction_digits"],
        },
        FormatterDescriptor {
            name: "percent",
            category: "Percentage",
            input_type: "Number | Decimal | Float",
            description: "Percentage formatting with fraction or whole scale and signed display",
            example_input: "0.25",
            example_output: "25%",
            options: vec!["scale (fraction/whole)", "minimum_fraction_digits", "signed"],
        },
        FormatterDescriptor {
            name: "date",
            category: "Date",
            input_type: "PlainDate | Instant | Date",
            description: "Calendar date formatting with Short, Medium, Long, Full styles or custom patterns",
            example_input: "2026-11-23",
            example_output: "Nov 23, 2026",
            options: vec!["style (short/medium/long/full)", "pattern", "year_only", "month_and_year"],
        },
        FormatterDescriptor {
            name: "time",
            category: "Time",
            input_type: "PlainTime | Instant",
            description: "Time formatting with 12/24 hour cycle, seconds, and fractional seconds",
            example_input: "14:30:00",
            example_output: "2:30 PM",
            options: vec!["style", "hour_cycle (12h/24h)", "include_seconds", "fractional_seconds"],
        },
        FormatterDescriptor {
            name: "datetime",
            category: "Date & Time",
            input_type: "PlainDateTime | ZonedDateTime | Instant",
            description: "Combined date and time presentation with time zone conversion",
            example_input: "2026-11-23T14:30:00Z",
            example_output: "Nov 23, 2026, 2:30 PM",
            options: vec!["date_style", "time_style", "time_zone", "include_time_zone_name"],
        },
        FormatterDescriptor {
            name: "relative",
            category: "Relative Time",
            input_type: "Instant | Timestamp",
            description: "Localized relative time (e.g. 'just now', '5 minutes ago', 'yesterday')",
            example_input: "-300 seconds",
            example_output: "5 minutes ago",
            options: vec!["style (natural/numeric)", "max_relative_days"],
        },
        FormatterDescriptor {
            name: "duration",
            category: "Duration",
            input_type: "Duration | Seconds",
            description: "Elapsed duration formatting in human ('1h 32m') or clock ('01:32:00') styles",
            example_input: "5528 seconds",
            example_output: "1 hour, 32 minutes",
            options: vec!["style (human/clock/short/narrow)", "max_units"],
        },
        FormatterDescriptor {
            name: "measurement",
            category: "Measurement",
            input_type: "Measurement | Quantity",
            description: "Physical unit measurement with localized plural forms and type-safe unit conversion",
            example_input: "100, Kilometer",
            example_output: "100 km",
            options: vec!["unit", "width (short/long)", "precision"],
        },
        FormatterDescriptor {
            name: "bytes",
            category: "File Size",
            input_type: "Bytes (u64)",
            description: "File size formatting in IEC binary (KiB, MiB) or SI decimal (kB, MB) units",
            example_input: "1572864",
            example_output: "1.5 MiB",
            options: vec!["standard (binary/decimal)", "precision"],
        },
        FormatterDescriptor {
            name: "list",
            category: "List",
            input_type: "List<String>",
            description: "Locale-aware list conjunction ('A, B, and C') and disjunction ('A, B, or C')",
            example_input: "[\"Alice\", \"Bob\", \"Charlie\"]",
            example_output: "Alice, Bob, and Charlie",
            options: vec!["type (conjunction/disjunction/unit)", "style"],
        },
    ]
}
