//! File size (bytes) and data rate formatting (SI base-1000 and IEC base-1024).
use crate::context::FormatContext;
use crate::decimal::Decimal;
use crate::error::Result;
use crate::number::{NumberOptions, format_number};
use serde::{Deserialize, Serialize};

/// Byte scaling standard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ByteStandard {
    #[default]
    Binary,  // IEC 1024: B, KiB, MiB, GiB, TiB
    Decimal, // SI 1000: B, kB, MB, GB, TB
}

/// Options configuring file size formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteOptions {
    pub standard: ByteStandard,
    pub maximum_fraction_digits: usize,
}

impl ByteOptions {
    pub fn binary() -> Self {
        Self {
            standard: ByteStandard::Binary,
            maximum_fraction_digits: 1,
        }
    }

    pub fn decimal() -> Self {
        Self {
            standard: ByteStandard::Decimal,
            maximum_fraction_digits: 1,
        }
    }

    pub fn precision(mut self, digits: usize) -> Self {
        self.maximum_fraction_digits = digits;
        self
    }
}

/// Format a byte count (e.g. 1536 -> "1.5 KiB" or "1.5 kB").
pub fn format_bytes(
    bytes: u64,
    options: &ByteOptions,
    context: &FormatContext,
) -> Result<String> {
    let (factor, units): (f64, &[&str]) = match options.standard {
        ByteStandard::Binary => (1024.0, &["B", "KiB", "MiB", "GiB", "TiB", "PiB"]),
        ByteStandard::Decimal => (1000.0, &["B", "kB", "MB", "GB", "TB", "PB"]),
    };

    let b = bytes as f64;
    let mut val = b;
    let mut idx = 0;

    while val >= factor && idx < units.len() - 1 {
        val /= factor;
        idx += 1;
    }

    let mut num_opts = NumberOptions::default();
    num_opts.minimum_fraction_digits = if idx == 0 { 0 } else { 0 };
    num_opts.maximum_fraction_digits = if idx == 0 { 0 } else { options.maximum_fraction_digits };

    let num_str = format_number(Decimal::from(val), &num_opts, context)?;
    Ok(format!("{num_str}\u{00A0}{}", units[idx]))
}

/// Data rate unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataRateUnit {
    #[default]
    BytesPerSecond, // B/s, kB/s, MB/s, GB/s
    BitsPerSecond,  // bps, kbps, Mbps, Gbps
}

/// Options configuring data rate formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataRateOptions {
    pub unit: DataRateUnit,
    pub maximum_fraction_digits: usize,
}

impl DataRateOptions {
    pub fn bytes_per_second() -> Self {
        Self {
            unit: DataRateUnit::BytesPerSecond,
            maximum_fraction_digits: 1,
        }
    }

    pub fn bits_per_second() -> Self {
        Self {
            unit: DataRateUnit::BitsPerSecond,
            maximum_fraction_digits: 1,
        }
    }
}

/// Format a data rate in raw units (bytes/sec or bits/sec).
pub fn format_data_rate(
    rate: f64,
    options: &DataRateOptions,
    context: &FormatContext,
) -> Result<String> {
    let units: &[&str] = match options.unit {
        DataRateUnit::BytesPerSecond => &["B/s", "kB/s", "MB/s", "GB/s", "TB/s"],
        DataRateUnit::BitsPerSecond => &["bps", "kbps", "Mbps", "Gbps", "Tbps"],
    };

    let mut val = rate;
    let mut idx = 0;
    while val >= 1000.0 && idx < units.len() - 1 {
        val /= 1000.0;
        idx += 1;
    }

    let mut num_opts = NumberOptions::default();
    num_opts.maximum_fraction_digits = if idx == 0 { 0 } else { options.maximum_fraction_digits };

    let num_str = format_number(Decimal::from(val), &num_opts, context)?;
    Ok(format!("{num_str}\u{00A0}{}", units[idx]))
}
