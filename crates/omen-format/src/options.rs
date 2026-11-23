//! Declarative formatting options enum and fluent builders for Omen view syntax.
use crate::address::AddressOptions;
use crate::bytes::{ByteOptions, ByteStandard, DataRateOptions, DataRateUnit};
use crate::context::CurrencyCode;
use crate::currency::CurrencyOptions;
use crate::date::{DateOptions, DateStyle};
use crate::datetime::DateTimeOptions;
use crate::duration::{DurationOptions, DurationStyle};
use crate::list::{ListOptions, ListType};
use crate::measurement::{MeasurementOptions, Unit};
use crate::number::NumberOptions;
use crate::percent::PercentOptions;
use crate::phone::{PhoneFormat, PhoneOptions};
use crate::relative::RelativeOptions;
use crate::text::{TextOptions, TextTransform};
use serde::{Deserialize, Serialize};

/// Comprehensive formatting option for declarative UI modifier `.format(...)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatOption {
    Number(NumberOptions),
    Currency(CurrencyOptions),
    Percent(PercentOptions),
    Date(DateOptions),
    Time(crate::time::TimeOptions),
    DateTime(DateTimeOptions),
    Relative(RelativeOptions),
    Duration(DurationOptions),
    Measurement(MeasurementOptions),
    Bytes(ByteOptions),
    DataRate(DataRateOptions),
    Text(TextOptions),
    List(ListOptions),
    Phone(PhoneOptions),
    Address(AddressOptions),
}

impl FormatOption {
    // --- Currency ---
    pub fn currency(code: &str) -> Self {
        let code = CurrencyCode::new(code).unwrap_or(CurrencyCode::USD);
        Self::Currency(CurrencyOptions::new(code))
    }

    // --- Number ---
    pub fn number() -> Self {
        Self::Number(NumberOptions::default())
    }

    pub fn number_compact() -> Self {
        Self::Number(NumberOptions::compact())
    }

    // --- Percent ---
    pub fn percent() -> Self {
        Self::Percent(PercentOptions::default())
    }

    pub fn percent_whole() -> Self {
        Self::Percent(PercentOptions::whole())
    }

    // --- Date ---
    pub fn date(style: DateStyle) -> Self {
        Self::Date(DateOptions {
            style: Some(style),
            ..DateOptions::default()
        })
    }

    pub fn date_pattern(pattern: impl Into<String>) -> Self {
        Self::Date(DateOptions::pattern(pattern))
    }

    // --- Time ---
    pub fn time(style: crate::time::TimeStyle) -> Self {
        Self::Time(crate::time::TimeOptions {
            style: Some(style),
            ..Default::default()
        })
    }

    // --- DateTime ---
    pub fn datetime(style: DateStyle) -> Self {
        match style {
            DateStyle::Short => Self::DateTime(DateTimeOptions::short()),
            DateStyle::Medium => Self::DateTime(DateTimeOptions::medium()),
            DateStyle::Long => Self::DateTime(DateTimeOptions::long()),
            DateStyle::Full => Self::DateTime(DateTimeOptions::full()),
        }
    }

    // --- Relative ---
    pub fn relative() -> Self {
        Self::Relative(RelativeOptions::default())
    }

    pub fn relative_numeric() -> Self {
        Self::Relative(RelativeOptions::numeric())
    }

    // --- Duration ---
    pub fn duration(style: DurationStyle) -> Self {
        Self::Duration(DurationOptions {
            style,
            ..Default::default()
        })
    }

    // --- Measurement ---
    pub fn measurement(unit: Unit) -> Self {
        Self::Measurement(MeasurementOptions::unit(unit))
    }

    // --- Bytes ---
    pub fn bytes(standard: ByteStandard) -> Self {
        Self::Bytes(ByteOptions {
            standard,
            maximum_fraction_digits: 1,
        })
    }

    // --- Data Rate ---
    pub fn data_rate(unit: DataRateUnit) -> Self {
        Self::DataRate(DataRateOptions {
            unit,
            maximum_fraction_digits: 1,
        })
    }

    // --- Text ---
    pub fn text(transform: TextTransform) -> Self {
        Self::Text(TextOptions { transform })
    }

    // --- List ---
    pub fn list(list_type: ListType) -> Self {
        Self::List(ListOptions {
            list_type,
            style: crate::list::ListStyle::Standard,
        })
    }

    // --- Phone ---
    pub fn phone(format: PhoneFormat) -> Self {
        Self::Phone(PhoneOptions {
            format,
            default_country_code: None,
        })
    }

    // --- Address ---
    pub fn address(multiline: bool) -> Self {
        Self::Address(AddressOptions {
            multiline,
            uppercase_country: true,
        })
    }

    /// Set an explicit time zone override for datetime formatting.
    pub fn time_zone(mut self, tz: impl Into<crate::context::TimeZoneId>) -> Self {
        if let Self::DateTime(ref mut dt) = self {
            dt.time_zone = Some(tz.into());
        }
        self
    }
}

impl Default for FormatOption {
    fn default() -> Self {
        Self::number()
    }
}

impl From<NumberOptions> for FormatOption {
    fn from(opts: NumberOptions) -> Self {
        Self::Number(opts)
    }
}

impl From<CurrencyOptions> for FormatOption {
    fn from(opts: CurrencyOptions) -> Self {
        Self::Currency(opts)
    }
}

impl From<PercentOptions> for FormatOption {
    fn from(opts: PercentOptions) -> Self {
        Self::Percent(opts)
    }
}

impl From<DateOptions> for FormatOption {
    fn from(opts: DateOptions) -> Self {
        Self::Date(opts)
    }
}

impl From<crate::time::TimeOptions> for FormatOption {
    fn from(opts: crate::time::TimeOptions) -> Self {
        Self::Time(opts)
    }
}

impl From<DateTimeOptions> for FormatOption {
    fn from(opts: DateTimeOptions) -> Self {
        Self::DateTime(opts)
    }
}

impl From<RelativeOptions> for FormatOption {
    fn from(opts: RelativeOptions) -> Self {
        Self::Relative(opts)
    }
}

impl From<DurationOptions> for FormatOption {
    fn from(opts: DurationOptions) -> Self {
        Self::Duration(opts)
    }
}

impl From<MeasurementOptions> for FormatOption {
    fn from(opts: MeasurementOptions) -> Self {
        Self::Measurement(opts)
    }
}

impl From<ByteOptions> for FormatOption {
    fn from(opts: ByteOptions) -> Self {
        Self::Bytes(opts)
    }
}

impl From<DataRateOptions> for FormatOption {
    fn from(opts: DataRateOptions) -> Self {
        Self::DataRate(opts)
    }
}

impl From<TextOptions> for FormatOption {
    fn from(opts: TextOptions) -> Self {
        Self::Text(opts)
    }
}

impl From<ListOptions> for FormatOption {
    fn from(opts: ListOptions) -> Self {
        Self::List(opts)
    }
}

impl From<PhoneOptions> for FormatOption {
    fn from(opts: PhoneOptions) -> Self {
        Self::Phone(opts)
    }
}

impl From<AddressOptions> for FormatOption {
    fn from(opts: AddressOptions) -> Self {
        Self::Address(opts)
    }
}
