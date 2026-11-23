//! Localized message formatting with named arguments, subformatters, and plural selection.
use crate::context::FormatContext;
use crate::currency::{CurrencyOptions, format_currency};
use crate::date::{DateOptions, PlainDate, format_date};
use crate::decimal::Decimal;
use crate::error::Result;
use crate::number::{NumberOptions, format_number};
use crate::plural::select_cardinal;
use std::collections::HashMap;

/// A typed argument that can be interpolated into a localized message.
#[derive(Clone, Debug)]
pub enum MessageArg {
    Text(String),
    Number(Decimal),
    Integer(i64),
    Currency(Decimal),
    Date(PlainDate),
}

impl From<&str> for MessageArg {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for MessageArg {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<i64> for MessageArg {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl From<i32> for MessageArg {
    fn from(n: i32) -> Self {
        Self::Integer(n as i64)
    }
}

impl From<u32> for MessageArg {
    fn from(n: u32) -> Self {
        Self::Integer(n as i64)
    }
}

impl From<usize> for MessageArg {
    fn from(n: usize) -> Self {
        Self::Integer(n as i64)
    }
}

impl From<f64> for MessageArg {
    fn from(f: f64) -> Self {
        Self::Number(Decimal::from(f))
    }
}

impl From<Decimal> for MessageArg {
    fn from(d: Decimal) -> Self {
        Self::Number(d)
    }
}

impl From<PlainDate> for MessageArg {
    fn from(d: PlainDate) -> Self {
        Self::Date(d)
    }
}

/// Format a message template with arguments and context.
/// Supports standard `{arg}` interpolation as well as `{count, plural, one {# item} other {# items}}`.
pub fn format_message(
    template: &str,
    args: &[(&str, MessageArg)],
    context: &FormatContext,
) -> Result<String> {
    let arg_map: HashMap<&str, &MessageArg> = args.iter().map(|(k, v)| (*k, v)).collect();
    let mut output = String::new();
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 1..];

        // Find closing brace matching depth
        let mut depth = 0;
        let mut close_idx = None;
        for (i, c) in tail.char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                if depth == 0 {
                    close_idx = Some(i);
                    break;
                } else {
                    depth -= 1;
                }
            }
        }

        let Some(end) = close_idx else {
            output.push_str(&rest[start..]);
            return Ok(output);
        };

        let inside = &tail[..end];
        rest = &tail[end + 1..];

        // Check if inside is a plural construct: e.g. "count, plural, one {# item} other {# items}"
        let parts: Vec<&str> = inside.splitn(3, ',').map(|s| s.trim()).collect();
        if parts.len() == 3 && parts[1] == "plural" {
            let var_name = parts[0];
            let count = match arg_map.get(var_name) {
                Some(MessageArg::Integer(n)) => *n,
                Some(MessageArg::Number(d)) => d.to_f64() as i64,
                _ => 0,
            };
            let category = select_cardinal(count, context.locale.as_str());
            let branch_content = extract_plural_branch(parts[2], category.as_str())
                .or_else(|| extract_plural_branch(parts[2], "other"))
                .unwrap_or("");
            let resolved = branch_content.replace('#', &count.to_string());
            output.push_str(&resolved);
        } else if parts.len() >= 2 {
            // Subformatted argument: e.g. "price, currency" or "created_at, date"
            let var_name = parts[0];
            let format_type = parts[1];
            if let Some(arg) = arg_map.get(var_name) {
                match (format_type, arg) {
                    ("currency", MessageArg::Number(d) | MessageArg::Currency(d)) => {
                        let curr_opts = context.currency
                            .map(CurrencyOptions::new)
                            .unwrap_or_else(|| CurrencyOptions::code("USD").unwrap());
                        output.push_str(&format_currency(*d, &curr_opts, context)?);
                    }
                    ("number", MessageArg::Number(d)) => {
                        output.push_str(&format_number(*d, &NumberOptions::default(), context)?);
                    }
                    ("date", MessageArg::Date(d)) => {
                        output.push_str(&format_date(*d, &DateOptions::medium(), context)?);
                    }
                    _ => output.push_str(&format_arg(arg, context)?),
                }
            }
        } else {
            // Simple interpolation: {name}
            let var_name = inside.trim();
            if let Some(arg) = arg_map.get(var_name) {
                output.push_str(&format_arg(arg, context)?);
            } else {
                output.push('{');
                output.push_str(inside);
                output.push('}');
            }
        }
    }

    output.push_str(rest);
    Ok(output)
}

fn format_arg(arg: &MessageArg, context: &FormatContext) -> Result<String> {
    match arg {
        MessageArg::Text(s) => Ok(s.clone()),
        MessageArg::Integer(n) => Ok(n.to_string()),
        MessageArg::Number(d) => format_number(*d, &NumberOptions::default(), context),
        MessageArg::Currency(d) => {
            let curr = context.currency.unwrap_or(crate::context::CurrencyCode::USD);
            format_currency(*d, &CurrencyOptions::new(curr), context)
        }
        MessageArg::Date(d) => format_date(*d, &DateOptions::medium(), context),
    }
}

fn extract_plural_branch<'a>(branches: &'a str, target_cat: &str) -> Option<&'a str> {
    let mut rest = branches;
    while let Some(start) = rest.find(target_cat) {
        let after_cat = &rest[start + target_cat.len()..];
        if let Some(open) = after_cat.find('{') {
            let inside = &after_cat[open + 1..];
            if let Some(close) = inside.find('}') {
                return Some(&inside[..close]);
            }
        }
        rest = after_cat;
    }
    None
}
