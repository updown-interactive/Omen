//! Structured postal address formatting respecting country conventions.
use crate::context::FormatContext;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// A structured postal address.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostalAddress {
    pub recipient: Option<String>,
    pub street: String,
    pub locality: String,            // City or town
    pub admin_area: Option<String>,  // State, province, or region
    pub postal_code: String,
    pub country_code: String,        // ISO 3166-1 alpha-2 (e.g. "US", "DE", "GB", "JP")
}

/// Options configuring address formatting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressOptions {
    pub uppercase_country: bool,
    pub multiline: bool,
}

impl AddressOptions {
    pub fn multiline() -> Self {
        Self {
            multiline: true,
            uppercase_country: true,
        }
    }

    pub fn single_line() -> Self {
        Self {
            multiline: false,
            uppercase_country: false,
        }
    }
}

/// Format a postal address according to destination country conventions.
pub fn format_address(
    address: &PostalAddress,
    options: &AddressOptions,
    _context: &FormatContext,
) -> Result<String> {
    let mut lines = Vec::new();

    if let Some(recipient) = &address.recipient {
        lines.push(recipient.clone());
    }

    let country = address.country_code.to_uppercase();
    match country.as_str() {
        // Japan: Postal Code -> Country/Prefecture/City/Street -> Recipient
        "JP" => {
            lines.clear();
            lines.push(format!("〒{}", address.postal_code));
            let mut line = String::new();
            if let Some(admin) = &address.admin_area {
                line.push_str(admin);
                line.push(' ');
            }
            line.push_str(&address.locality);
            line.push(' ');
            line.push_str(&address.street);
            lines.push(line);
            if let Some(recipient) = &address.recipient {
                lines.push(format!("{recipient} 様"));
            }
        }
        // Germany / France / Continental Europe: Street -> PostalCode Locality -> Country
        "DE" | "FR" | "IT" | "ES" | "NL" | "BE" | "AT" | "CH" => {
            lines.push(address.street.clone());
            lines.push(format!("{} {}", address.postal_code, address.locality));
            lines.push(country.clone());
        }
        // UK: Street -> Locality -> PostalCode -> Country
        "GB" => {
            lines.push(address.street.clone());
            lines.push(address.locality.clone());
            lines.push(address.postal_code.clone());
            lines.push("UNITED KINGDOM".to_string());
        }
        // US, Canada, India, Australia, Default: Street -> Locality, AdminArea PostalCode -> Country
        _ => {
            lines.push(address.street.clone());
            let mut city_line = address.locality.clone();
            if let Some(admin) = &address.admin_area {
                city_line.push_str(", ");
                city_line.push_str(admin);
            }
            city_line.push(' ');
            city_line.push_str(&address.postal_code);
            lines.push(city_line);
            lines.push(country.clone());
        }
    }

    let sep = if options.multiline { "\n" } else { ", " };
    Ok(lines.join(sep))
}
