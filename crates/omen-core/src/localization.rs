//! Observable message catalogs with explicit fallback and layout direction.
use crate::Value;
use std::{collections::BTreeMap, rc::Rc};
/// Inline layout direction. Text shaping remains the native renderer's responsibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutDirection {
    /// Left-to-right inline progression.
    #[default]
    LeftToRight,
    /// Right-to-left inline progression.
    RightToLeft,
}
/// A locale identifier and its layout direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale {
    /// BCP-47-style locale identifier.
    pub id: String,
    /// Direction for logical leading/trailing placement.
    pub direction: LayoutDirection,
}
impl Locale {
    /// Construct a locale; common RTL language subtags select RTL automatically.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let language = id.split(['-', '_']).next().unwrap_or_default();
        let direction = if matches!(
            language.to_ascii_lowercase().as_str(),
            "ar" | "he" | "fa" | "ur" | "ps" | "dv" | "yi"
        ) {
            LayoutDirection::RightToLeft
        } else {
            LayoutDirection::LeftToRight
        };
        Self { id, direction }
    }
}
/// Translation catalogs keyed by locale then message key.
#[derive(Clone, Debug)]
pub struct Localizer {
    locale: Value<Locale>,
    fallback: String,
    catalogs: Rc<BTreeMap<String, BTreeMap<String, String>>>,
}
impl Localizer {
    /// Start a catalog with an observable current locale and fallback locale.
    pub fn new(locale: Locale, fallback: impl Into<String>) -> Self {
        Self {
            locale: Value::new(locale),
            fallback: fallback.into(),
            catalogs: Rc::new(BTreeMap::new()),
        }
    }
    /// Declare an available locale, including an entirely fallback-based catalog.
    pub fn catalog_locale(mut self, locale: impl Into<String>) -> Self {
        Rc::make_mut(&mut self.catalogs)
            .entry(locale.into())
            .or_default();
        self
    }
    /// Add or replace a message before sharing the localizer.
    pub fn message(
        mut self,
        locale: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Rc::make_mut(&mut self.catalogs)
            .entry(locale.into())
            .or_default()
            .insert(key.into(), value.into());
        self
    }
    /// Current locale; reads track reactive dependencies.
    pub fn locale(&self) -> Locale {
        self.locale.get()
    }
    /// Change locale for all clones of this localizer.
    pub fn set_locale(&self, locale: Locale) {
        self.locale.set(locale);
    }
    /// Resolve exact locale, then language, then fallback; missing keys display the key.
    pub fn text(&self, key: &str) -> String {
        let locale = self.locale.get();
        let language = locale.id.split(['-', '_']).next().unwrap_or_default();
        [&locale.id, language, &self.fallback]
            .into_iter()
            .find_map(|id| self.catalogs.get(id).and_then(|catalog| catalog.get(key)))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
    /// Substitute named `{argument}` placeholders in one pass, without reinterpreting values.
    pub fn format(&self, key: &str, arguments: &[(&str, &str)]) -> String {
        let text = self.text(key);
        interpolate(&text, arguments)
    }
}

/// An unsupported locale selection for a typed translation resource.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unsupported locale: {0}")]
pub struct UnsupportedLocale(pub String);
impl Localizer {
    /// Select a declared locale, or a regional variant of a declared language.
    pub fn try_set_locale(&self, locale: Locale) -> Result<(), UnsupportedLocale> {
        let language = locale.id.split(['-', '_']).next().unwrap_or_default();
        if !self.catalogs.contains_key(&locale.id) && !self.catalogs.contains_key(language) {
            return Err(UnsupportedLocale(locale.id));
        }
        self.set_locale(locale);
        Ok(())
    }
    /// Format an integer plural message with named parameters and `{count}`.
    /// Uses CLDR cardinal rules through `intl_pluralrules`.
    /// Missing categories resolve to `other` in the selected catalog before fallback.
    pub fn plural(&self, key: &str, count: i64, arguments: &[(&str, &str)]) -> String {
        let locale = self.locale();
        let language = locale.id.split(['-', '_']).next().unwrap_or_default();
        let resolved = [&locale.id, language, &self.fallback]
            .into_iter()
            .find_map(|id| {
                let category = plural_category(id, count);
                let selected = format!("{key}.{category}");
                let other = format!("{key}.other");
                self.catalogs
                    .get(id)
                    .and_then(|catalog| catalog.get(&selected).or_else(|| catalog.get(&other)))
            })
            .cloned()
            .unwrap_or_else(|| key.to_string());
        let count = count.to_string();
        let mut parameters = arguments.to_vec();
        parameters.push(("count", &count));
        interpolate(&resolved, &parameters)
    }
}
fn interpolate(text: &str, arguments: &[(&str, &str)]) -> String {
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('}') else {
            output.push_str(&rest[start..]);
            return output;
        };
        if let Some((_, value)) = arguments.iter().find(|(name, _)| *name == &tail[..end]) {
            output.push_str(value);
        } else {
            output.push_str(&rest[start..start + end + 2]);
        }
        rest = &tail[end + 1..];
    }
    output.push_str(rest);
    output
}

fn plural_category(locale: &str, count: i64) -> &'static str {
    use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
    let locale = locale.replace('_', "-");
    let language = locale.split('-').next().unwrap_or_default();
    let category = [locale.as_str(), language].into_iter().find_map(|id| {
        let id = id.parse::<unic_langid::LanguageIdentifier>().ok()?;
        PluralRules::create(id, PluralRuleType::CARDINAL)
            .ok()?
            .select(count.unsigned_abs())
            .ok()
    });
    match category {
        Some(PluralCategory::ZERO) => "zero",
        Some(PluralCategory::ONE) => "one",
        Some(PluralCategory::TWO) => "two",
        Some(PluralCategory::FEW) => "few",
        Some(PluralCategory::MANY) => "many",
        _ => "other",
    }
}
