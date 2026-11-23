//! Local language help available before rust-analyzer responds.
use omen::TextSnapshot;
use omen_macro_core::{
    editor::{is_code_at, is_completion_context},
    inspector::MODIFIERS,
};
use std::sync::OnceLock;

pub(crate) fn omen_help(text: &TextSnapshot, cursor: usize) -> Option<String> {
    let source = text.slice(..);
    let (word, before, after) = word_at(&source, cursor)?;
    if !is_code_at(&source, before.len()) {
        return None;
    }
    if after.trim_start().starts_with('!') {
        return match word {
            "view" => Some(help(
                "view!",
                "Define a named Omen view or build an anonymous view.",
                "view! {\n    @Design\n    pub Home {\n        Text(\"Hello, Omen!\")\n    }\n}",
            )),
            "main" => Some(help(
                "main!",
                "Declare the root view and generate the entry point for the target platform.",
                "main! { Home() }",
            )),
            "environment" => Some(help(
                "environment!",
                "Declare a shared environment model that named views can read.",
                "environment! {\n    pub AppEnvironment {\n        @Reactive count: i32 = 0;\n    }\n}",
            )),
            "locale" => Some(help(
                "locale!",
                "Define translated strings for views that use @Localize.",
                "locale! {\n    pub AppStrings {\n        en { title: \"Welcome\" }\n    }\n}",
            )),
            _ => None,
        };
    }
    if before.ends_with('@') && is_completion_context(&source, before.len() + word.len()) {
        return match word {
            "Design" => Some(help(
                "@Design",
                "Enable Design and Split for this file and choose the view Studio renders by default.",
                "view! {\n    @Design\n    Home { Text(\"Hello\") }\n}",
            )),
            "State" => Some(help(
                "@State",
                "Store reactive state for this view instance. Use a dollar binding to connect it to an input.",
                "@State count = 0",
            )),
            "Environment" => Some(help(
                "@Environment",
                "Read a typed value from the enclosing environment.",
                "@Environment env",
            )),
            "Responsive" => Some(help(
                "@Responsive",
                "Read the available layout width and its compact, medium, or expanded category.",
                "@Responsive layout",
            )),
            "Localize" => Some(help(
                "@Localize",
                "Load a translation catalog for the current locale.",
                "@Localize strings = AppStrings()",
            )),
            _ => None,
        };
    }
    if before.trim_end().ends_with('.')
        && omen_macro_core::formatter::modifier_ranges(&source)
            .iter()
            .any(|range| range.start == before.len())
        && let Some(modifier) = MODIFIERS.iter().find(|modifier| modifier.name == word)
    {
        let description = if modifier.components.is_empty() {
            format!(
                "{} is a view modifier in the {} category.",
                modifier.label,
                modifier.category.to_lowercase()
            )
        } else {
            format!(
                "{} applies to {}.",
                modifier.label,
                modifier.components.join(", ")
            )
        };
        return Some(help(
            &format!(".{word}"),
            &description,
            &format!(".{word}({})", modifier.default),
        ));
    }
    if !is_completion_context(&source, before.len() + word.len()) {
        return None;
    }
    static ITEMS: OnceLock<Vec<super::library::LibraryItem>> = OnceLock::new();
    let item = ITEMS
        .get_or_init(|| super::library::items("", 0))
        .iter()
        .find(|item| item.snippet.name == word)?;
    Some(help(
        word,
        &format!("{}\n\n{}", item.summary, item.documentation),
        &item.snippet.insert,
    ))
}

pub(crate) fn component_help<'a>(
    text: &TextSnapshot,
    cursor: usize,
    project: impl FnOnce() -> &'a [omen_macro_core::editor::DslComponent],
) -> Option<String> {
    let source = text.slice(..);
    let (word, before, _) = word_at(&source, cursor)?;
    if !word.starts_with(char::is_uppercase)
        || !is_completion_context(&source, before.len() + word.len())
    {
        return None;
    }
    let local = omen_macro_core::editor::components(&source);
    let view = local
        .iter()
        .find(|view| view.name == word)
        .or_else(|| project().iter().find(|view| view.name == word))?;
    let arguments = view
        .properties
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join(", ");
    let description = if view.properties.is_empty() {
        "A project view with no properties.".to_owned()
    } else {
        format!(
            "Properties: {}",
            view.properties
                .iter()
                .map(|(name, _)| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Some(help(word, &description, &format!("{word}({arguments})")))
}

pub(crate) fn rust_help(text: &TextSnapshot, cursor: usize) -> Option<String> {
    let row = text.byte_to_line(cursor);
    let line = text.line(row);
    let (word, before, _) = word_at(&line, cursor.checked_sub(text.line_start(row))?)?;
    let (description, example) = match word {
        "let" => (
            "Bind a value to a name. Add mut to allow reassignment.",
            "let mut count = 0;",
        ),
        "fn" => (
            "Declare a function. Use -> to specify its return type.",
            "fn double(value: i32) -> i32 {\n    value * 2\n}",
        ),
        "use" => (
            "Bring a path into the current scope.",
            "use std::collections::HashMap;",
        ),
        "pub" => (
            "Make an item visible outside its module.",
            "pub fn greeting() -> &'static str { \"Hello\" }",
        ),
        "struct" => (
            "Define a type with named or positional fields.",
            "struct Point { x: f32, y: f32 }",
        ),
        "enum" => (
            "Define a type with a fixed set of variants.",
            "enum Direction { Left, Right }",
        ),
        "match" => (
            "Select an arm by matching a value against patterns.",
            "match value {\n    Some(item) => item,\n    None => 0,\n}",
        ),
        "if" => (
            "Run a branch when a condition is true. The branches can produce a value.",
            "let label = if ready { \"Start\" } else { \"Wait\" };",
        ),
        "impl" => (
            "Define methods for a type or implement a trait for it.",
            "impl Point {\n    fn x(&self) -> f32 { self.x }\n}",
        ),
        "async" => (
            "Create a future that can suspend at an await point.",
            "async fn load() { fetch().await; }",
        ),
        "await" => (
            "Suspend the current future until another future is ready.",
            "let result = fetch().await;",
        ),
        _ => return None,
    };
    let source = text.slice(..);
    is_code_at(&source, text.line_start(row) + before.len())
        .then(|| help(word, description, example))
}

fn word_at(source: &str, mut cursor: usize) -> Option<(&str, &str, &str)> {
    if source.as_bytes().get(cursor) == Some(&b'@') {
        cursor += 1;
    }
    let before = source.get(..cursor)?;
    let after = source.get(cursor..)?;
    let word_char = |ch: char| ch.is_alphanumeric() || ch == '_';
    let start = before.trim_end_matches(word_char).len();
    let end = source.len() - after.trim_start_matches(word_char).len();
    (start < end).then(|| (&source[start..end], &source[..start], &source[end..]))
}

fn help(title: &str, description: &str, example: &str) -> String {
    format!("## {title}\n\n{description}\n\n### Example\n\n```rust\n{example}\n```")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_components_have_local_hover_examples() {
        for name in omen_macro_core::editor::BUILTIN_VIEW_NAMES {
            let source = format!("view! {{ @Design Home {{ {name}() }} }}");
            let cursor = source.find(name).unwrap();
            let help = omen_help(&source.into(), cursor)
                .unwrap_or_else(|| panic!("missing hover for {name}"));
            assert!(help.contains(&format!("## {name}\n")));
            assert!(help.contains("```rust"));
        }
    }

    #[test]
    fn project_view_help_includes_a_call_with_named_properties() {
        let source = "view! { Home { UserCard(title: \"Ada\") } }";
        let text = TextSnapshot::from(source);
        let components = omen_macro_core::editor::components(
            "view! { pub UserCard(title: String) { Text(title) } }",
        );
        let documentation =
            component_help(&text, source.find("UserCard").unwrap(), || &components).unwrap();
        assert!(documentation.contains("Properties: `title`"));
        assert!(documentation.contains("UserCard(title: \"value\")"));
    }

    #[test]
    fn local_hover_distinguishes_code_from_comments_and_strings() {
        let source = "// view!\nview! { @Design Home { Text(\"view!\").padding(4) } }";
        let text = TextSnapshot::from(source);
        assert!(omen_help(&text, source.find("view!").unwrap()).is_none());
        assert!(omen_help(&text, source.rfind("view!").unwrap()).is_none());
        assert!(
            omen_help(&text, source.find("@Design").unwrap())
                .unwrap()
                .contains("Design and Split")
        );
        assert!(
            omen_help(&text, source.find("padding").unwrap())
                .unwrap()
                .contains(".padding(")
        );
    }
}
