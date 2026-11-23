//! Physical measurement formatting and type-safe dimension conversion.
use crate::context::FormatContext;
use crate::decimal::Decimal;
use crate::error::{FormatError, Result};
use crate::number::{NumberOptions, format_number};
use serde::{Deserialize, Serialize};

/// Supported physical units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    // Length
    Meter,
    Kilometer,
    Centimeter,
    Millimeter,
    Mile,
    Yard,
    Foot,
    Inch,

    // Mass
    Gram,
    Kilogram,
    Milligram,
    Pound,
    Ounce,

    // Temperature
    Celsius,
    Fahrenheit,
    Kelvin,

    // Speed
    KilometersPerHour,
    MilesPerHour,
    MetersPerSecond,

    // Volume
    Liter,
    Milliliter,
    Gallon,
    FluidOunce,

    // Pressure
    Pascal,
    Bar,
    Psi,

    // Power & Energy
    Watt,
    Kilowatt,
    Joule,
    KilowattHour,

    // Frequency
    Hertz,
    Kilohertz,
    Megahertz,
    Gigahertz,

    // Angle
    Degree,
    Radian,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Meter => "m",
            Self::Kilometer => "km",
            Self::Centimeter => "cm",
            Self::Millimeter => "mm",
            Self::Mile => "mi",
            Self::Yard => "yd",
            Self::Foot => "ft",
            Self::Inch => "in",
            Self::Gram => "g",
            Self::Kilogram => "kg",
            Self::Milligram => "mg",
            Self::Pound => "lb",
            Self::Ounce => "oz",
            Self::Celsius => "°C",
            Self::Fahrenheit => "°F",
            Self::Kelvin => "K",
            Self::KilometersPerHour => "km/h",
            Self::MilesPerHour => "mph",
            Self::MetersPerSecond => "m/s",
            Self::Liter => "L",
            Self::Milliliter => "mL",
            Self::Gallon => "gal",
            Self::FluidOunce => "fl oz",
            Self::Pascal => "Pa",
            Self::Bar => "bar",
            Self::Psi => "psi",
            Self::Watt => "W",
            Self::Kilowatt => "kW",
            Self::Joule => "J",
            Self::KilowattHour => "kWh",
            Self::Hertz => "Hz",
            Self::Kilohertz => "kHz",
            Self::Megahertz => "MHz",
            Self::Gigahertz => "GHz",
            Self::Degree => "°",
            Self::Radian => "rad",
        }
    }

    pub fn name(self, _lang: &str, plural: bool) -> &'static str {
        match self {
            Self::Kilometer => if plural { "kilometers" } else { "kilometer" },
            Self::Meter => if plural { "meters" } else { "meter" },
            Self::Mile => if plural { "miles" } else { "mile" },
            Self::Kilogram => if plural { "kilograms" } else { "kilogram" },
            Self::Pound => if plural { "pounds" } else { "pound" },
            Self::Celsius => "degrees Celsius",
            Self::Fahrenheit => "degrees Fahrenheit",
            _ => self.symbol(),
        }
    }

    /// Convert a value from this unit to another unit in the same dimension.
    pub fn convert(self, value: f64, target: Unit) -> Result<f64> {
        if self == target {
            return Ok(value);
        }

        // Temperature affine conversion
        match (self, target) {
            (Self::Celsius, Self::Fahrenheit) => return Ok(value * 9.0 / 5.0 + 32.0),
            (Self::Fahrenheit, Self::Celsius) => return Ok((value - 32.0) * 5.0 / 9.0),
            (Self::Celsius, Self::Kelvin) => return Ok(value + 273.15),
            (Self::Kelvin, Self::Celsius) => return Ok(value - 273.15),
            (Self::Fahrenheit, Self::Kelvin) => return Ok((value - 32.0) * 5.0 / 9.0 + 273.15),
            (Self::Kelvin, Self::Fahrenheit) => return Ok((value - 273.15) * 9.0 / 5.0 + 32.0),
            _ => {}
        }

        // Multiplicative factor to SI base
        let to_base = match self {
            // Length (base: meter)
            Self::Meter => 1.0,
            Self::Kilometer => 1000.0,
            Self::Centimeter => 0.01,
            Self::Millimeter => 0.001,
            Self::Mile => 1609.344,
            Self::Yard => 0.9144,
            Self::Foot => 0.3048,
            Self::Inch => 0.0254,

            // Mass (base: gram)
            Self::Gram => 1.0,
            Self::Kilogram => 1000.0,
            Self::Milligram => 0.001,
            Self::Pound => 453.59237,
            Self::Ounce => 28.349523125,

            // Speed (base: m/s)
            Self::MetersPerSecond => 1.0,
            Self::KilometersPerHour => 1.0 / 3.6,
            Self::MilesPerHour => 0.44704,

            // Volume (base: liter)
            Self::Liter => 1.0,
            Self::Milliliter => 0.001,
            Self::Gallon => 3.785411784,
            Self::FluidOunce => 0.0295735295625,

            _ => {
                return Err(FormatError::InvalidUnitConversion {
                    from: format!("{:?}", self),
                    to: format!("{:?}", target),
                });
            }
        };

        let from_base = match target {
            Self::Meter => 1.0,
            Self::Kilometer => 0.001,
            Self::Centimeter => 100.0,
            Self::Millimeter => 1000.0,
            Self::Mile => 1.0 / 1609.344,
            Self::Yard => 1.0 / 0.9144,
            Self::Foot => 1.0 / 0.3048,
            Self::Inch => 1.0 / 0.0254,

            Self::Gram => 1.0,
            Self::Kilogram => 0.001,
            Self::Milligram => 1000.0,
            Self::Pound => 1.0 / 453.59237,
            Self::Ounce => 1.0 / 28.349523125,

            Self::MetersPerSecond => 1.0,
            Self::KilometersPerHour => 3.6,
            Self::MilesPerHour => 1.0 / 0.44704,

            Self::Liter => 1.0,
            Self::Milliliter => 1000.0,
            Self::Gallon => 1.0 / 3.785411784,
            Self::FluidOunce => 1.0 / 0.0295735295625,

            _ => {
                return Err(FormatError::InvalidUnitConversion {
                    from: format!("{:?}", self),
                    to: format!("{:?}", target),
                });
            }
        };

        Ok(value * to_base * from_base)
    }
}

/// Unit name display width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitWidth {
    #[default]
    Short, // "km", "lb", "°C"
    Long,  // "kilometers", "pounds"
    Narrow, // "km", "lb"
}

/// A measured quantity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub value: f64,
    pub unit: Unit,
}

impl Measurement {
    pub fn new(value: impl Into<f64>, unit: Unit) -> Self {
        Self {
            value: value.into(),
            unit,
        }
    }

    pub fn kilometers(value: impl Into<f64>) -> Self {
        Self::new(value, Unit::Kilometer)
    }

    pub fn meters(value: impl Into<f64>) -> Self {
        Self::new(value, Unit::Meter)
    }

    pub fn miles(value: impl Into<f64>) -> Self {
        Self::new(value, Unit::Mile)
    }

    pub fn celsius(value: impl Into<f64>) -> Self {
        Self::new(value, Unit::Celsius)
    }

    pub fn fahrenheit(value: impl Into<f64>) -> Self {
        Self::new(value, Unit::Fahrenheit)
    }

    pub fn convert_to(&self, target_unit: Unit) -> Result<Self> {
        let converted = self.unit.convert(self.value, target_unit)?;
        Ok(Self::new(converted, target_unit))
    }
}

/// Options configuring measurement formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementOptions {
    pub unit: Option<Unit>,
    pub width: UnitWidth,
    pub minimum_fraction_digits: usize,
    pub maximum_fraction_digits: usize,
}

impl MeasurementOptions {
    pub fn unit(unit: Unit) -> Self {
        Self {
            unit: Some(unit),
            width: UnitWidth::Short,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 2,
        }
    }

    pub fn long(mut self) -> Self {
        self.width = UnitWidth::Long;
        self
    }

    pub fn precision(mut self, min: usize, max: usize) -> Self {
        self.minimum_fraction_digits = min;
        self.maximum_fraction_digits = max;
        self
    }
}

/// Format a measurement.
pub fn format_measurement(
    measurement: Measurement,
    options: &MeasurementOptions,
    context: &FormatContext,
) -> Result<String> {
    let target = options.unit.unwrap_or(measurement.unit);
    let converted_val = measurement.unit.convert(measurement.value, target)?;

    let mut num_opts = NumberOptions::default();
    num_opts.minimum_fraction_digits = options.minimum_fraction_digits;
    num_opts.maximum_fraction_digits = options.maximum_fraction_digits;

    let num_str = format_number(Decimal::from(converted_val), &num_opts, context)?;

    let is_plural = (converted_val.abs() - 1.0).abs() > 0.0001;
    let unit_str = match options.width {
        UnitWidth::Long => target.name(context.locale.language(), is_plural),
        _ => target.symbol(),
    };

    let space = if matches!(target, Unit::Degree | Unit::Celsius | Unit::Fahrenheit) && options.width != UnitWidth::Long {
        if target == Unit::Degree { "" } else { "\u{00A0}" }
    } else {
        "\u{00A0}"
    };

    Ok(format!("{num_str}{space}{unit_str}"))
}
