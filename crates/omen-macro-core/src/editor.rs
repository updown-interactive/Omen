//! Compiler-owned vocabulary for lightweight Studio DSL completion.
mod declarations;
use proc_macro2::{TokenStream, TokenTree};
use std::ops::Range;

/// Built-in view names recognized by the compiler and offered by Studio.
pub const BUILTIN_VIEW_NAMES: &[&str] = &[
    "NavigationStack",
    "NavigationBar",
    "TabView",
    "Tab",
    "Text",
    "Button",
    "Layout",
    "AdaptiveLayout",
    "AdaptiveGrid",
    "List",
    "Image",
    "Icon",
    "TextField",
    "TextEditor",
    "SecureField",
    "Checkbox",
    "Toggle",
    "Card",
    "Progress",
    "Divider",
    "Gap",
];
/// An auxiliary source edit applied atomically with a completion.
#[derive(Clone, Debug)]
pub struct CompletionEdit {
    /// Original UTF-8 range.
    pub range: Range<usize>,
    /// Replacement or insertion source.
    pub insert: String,
}
/// Declaration shortcuts at a Rust module item position.
pub fn declaration_completions(source: &str, cursor: usize) -> Vec<DslCompletion> {
    declarations::completions(source, cursor)
}
/// A component or directive completion with plain defaults and editable tab stops.
#[derive(Clone, Debug)]
pub struct DslCompletion {
    /// Component or declaration name shown in the suggestion list.
    pub name: String,
    /// UTF-8 range of the identifier being completed.
    pub range: Range<usize>,
    /// Complete Omen source inserted on acceptance.
    pub insert: String,
    /// Editable UTF-8 ranges relative to `insert`, ending with the final caret.
    pub placeholders: Vec<Range<usize>>,
    /// Additional import edits in the same source snapshot.
    pub additional_edits: Vec<CompletionEdit>,
}
/// A reusable view signature discovered in a source file.
#[derive(Clone, Debug)]
pub struct DslComponent {
    /// Public DSL component name.
    pub name: String,
    /// Property names and suggested Rust expressions, in declaration order.
    pub properties: Vec<(String, String)>,
}
fn declared_views(tokens: TokenStream, output: &mut Vec<DslComponent>) {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    for part in tokens.windows(3) {
        if matches!(&part[0],TokenTree::Ident(name) if name=="view")
            && matches!(&part[1],TokenTree::Punct(punct) if punct.as_char()=='!')
            && let TokenTree::Group(group) = &part[2]
            && crate::is_view_declaration(group.stream())
            && let Ok(views) = syn::parse2::<crate::ast::NamedViews>(group.stream())
        {
            for view in views.0 {
                output.push(DslComponent {
                    name: view.name.to_string(),
                    properties: view
                        .properties
                        .iter()
                        .map(|property| (property.name.to_string(), property_default(&property.ty)))
                        .collect(),
                });
            }
        }
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            declared_views(group.stream(), output);
        }
    }
}
fn property_default(ty: &syn::Type) -> String {
    if let syn::Type::Reference(reference) = ty {
        return property_default(&reference.elem);
    }
    let name = if let syn::Type::Path(path) = ty {
        path.path
            .segments
            .last()
            .map(|part| part.ident.to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    match name.as_str() {
        "String" | "str" => "\"value\"".into(),
        "bool" => "false".into(),
        "usize" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32"
        | "i64" | "i128" => format!("0{name}"),
        "f32" | "f64" => format!("0.0{name}"),
        "Option" => "None".into(),
        "Vec" => "Vec::new()".into(),
        "Binding" | "Value" | "TextValue" | "BoolValue" | "Observable" => "$value".into(),
        "Action" => "Action::new(|| {})".into(),
        _ => "todo!()".into(),
    }
}
/// Extract reusable view signatures from complete source.
pub fn components(source: &str) -> Vec<DslComponent> {
    let mut views = Vec::new();
    if let Ok(tokens) = source.parse::<TokenStream>() {
        declared_views(tokens, &mut views);
    }
    views
}
// Mask literals and comments without changing byte offsets. This keeps braces,
// fake view! invocations, and identifiers in raw strings out of completion context.
fn code_mask(source: &str) -> Vec<u8> {
    let bytes = source.as_bytes();
    let mut mask = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let end = if bytes[i..].starts_with(b"//") {
            bytes[i..]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |n| i + n)
        } else if bytes[i..].starts_with(b"/*") {
            let mut n = i + 2;
            let mut depth = 1;
            while n < bytes.len() && depth > 0 {
                if bytes[n..].starts_with(b"/*") {
                    depth += 1;
                    n += 2;
                } else if bytes[n..].starts_with(b"*/") {
                    depth -= 1;
                    n += 2;
                } else {
                    n += 1;
                }
            }
            n
        } else if bytes[i] == b'r' && {
            let mut n = i + 1;
            while bytes.get(n) == Some(&b'#') {
                n += 1;
            }
            bytes.get(n) == Some(&b'"')
        } {
            let mut n = i + 1;
            while bytes.get(n) == Some(&b'#') {
                n += 1;
            }
            let hashes = n - i - 1;
            n += 1;
            loop {
                if n >= bytes.len() {
                    break bytes.len();
                }
                if bytes[n] == b'"'
                    && bytes
                        .get(n + 1..n + 1 + hashes)
                        .is_some_and(|tail| tail.iter().all(|b| *b == b'#'))
                {
                    break n + 1 + hashes;
                }
                n += 1;
            }
        } else if bytes[i] == b'"'
            || bytes[i] == b'\'' && {
                let rest = &source[i + 1..];
                rest.starts_with('\\')
                    || rest
                        .chars()
                        .next()
                        .is_some_and(|ch| bytes.get(i + 1 + ch.len_utf8()) == Some(&b'\''))
            }
        {
            let quote = bytes[i];
            let mut n = i + 1;
            while n < bytes.len() {
                if bytes[n] == b'\\' {
                    n = (n + 2).min(bytes.len());
                } else if bytes[n] == quote {
                    n += 1;
                    break;
                } else {
                    n += 1;
                }
            }
            n
        } else {
            i += 1;
            continue;
        };
        for byte in &mut mask[start..end] {
            if *byte != b'\n' {
                *byte = 0;
            }
        }
        i = end;
    }
    mask
}
fn context(source: &str, cursor: usize) -> Option<(Range<usize>, bool, usize)> {
    if cursor > source.len() || !source.is_char_boundary(cursor) {
        return None;
    }
    let mask = code_mask(source);
    if cursor > 0
        && mask[cursor - 1] != source.as_bytes()[cursor - 1]
        && (cursor == source.len() || mask[cursor] != source.as_bytes()[cursor])
    {
        return None;
    }
    let code = std::str::from_utf8(&mask).ok()?;
    if cursor < source.len() && mask[cursor] == 0 {
        return None;
    }
    let before = &code[..cursor];
    let macro_start = before.rfind("view!")?;
    let open = before[macro_start..].find('{')? + macro_start;
    let depth = before[open..].bytes().fold(0_i64, |depth, b| {
        depth
            + if b == b'{' {
                1
            } else if b == b'}' {
                -1
            } else {
                0
            }
    });
    if depth <= 0 {
        return None;
    }
    let mut delimiters = Vec::new();
    for byte in before[open..].bytes() {
        match byte {
            b'{' | b'(' | b'[' => delimiters.push(byte),
            b'}' | b')' | b']' => {
                delimiters.pop();
            }
            _ => {}
        }
    }
    if delimiters.last() != Some(&b'{') {
        return None;
    }
    let start = cursor
        - source[..cursor]
            .chars()
            .rev()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
    if mask[start..cursor] != source.as_bytes()[start..cursor] {
        return None;
    }
    let directive = source[..start].ends_with('@');
    if !directive
        && source[start..cursor]
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_uppercase())
    {
        return None;
    }
    if !directive && source[..start].trim_end().ends_with(['.', ':']) {
        return None;
    }
    let end = cursor
        + source[cursor..]
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
    Some((start..end, directive, macro_start))
}
/// Whether the byte offset belongs to code rather than a string or comment.
pub fn is_code_at(source: &str, offset: usize) -> bool {
    source
        .as_bytes()
        .get(offset)
        .is_some_and(|byte| code_mask(source)[offset] == *byte)
}

/// Whether a cursor is in a component/directive completion context.
pub fn is_completion_context(source: &str, cursor: usize) -> bool {
    context(source, cursor).is_some()
}
#[derive(Default)]
struct Snippet {
    text: String,
    stops: Vec<Range<usize>>,
}
impl Snippet {
    fn text(&mut self, value: &str) {
        self.text.push_str(value);
    }
    fn slot(&mut self, value: &str) {
        let start = self.text.len();
        self.text(value);
        self.stops.push(start..self.text.len());
    }
    fn template(value: &str) -> Self {
        let mut snippet = Self::default();
        let mut rest = value;
        while let Some(start) = rest.find("${") {
            snippet.text(&rest[..start]);
            let tail = &rest[start + 2..];
            let Some(end) = tail.find('}') else {
                snippet.text(&rest[start..]);
                return snippet;
            };
            snippet.slot(&tail[..end]);
            rest = &tail[end + 1..];
        }
        snippet.text(rest);
        snippet
    }
}
fn builtin(name: &str) -> Option<Snippet> {
    Some(Snippet::template(match name {
        "Text" => "Text(\"${Text}\")",
        "Button" => "Button(\"${Button}\") {\n    ${// Action}\n}",
        "Layout" => "Layout {\n    ${Text(\"Content\")}\n}",
        "Card" => "Card {\n    ${Text(\"Content\")}\n}",
        "NavigationStack" => "NavigationStack($${navigator}) {\n    ${Text(\"Content\")}\n}",
        "NavigationBar" => "NavigationBar(\"${Title}\") {\n    ${Text(\"Content\")}\n}",
        "TabView" => {
            "TabView($${selection}) {\n    Tab(\"${tab}\", \"${Title}\") {\n        ${Text(\"Content\")}\n    }\n}"
        }
        "Tab" => "Tab(\"${tab}\", \"${Title}\") {\n    ${Text(\"Content\")}\n}",
        "List" => {
            "List(${items}, id: |item| item.to_string()) {\n    item in ${Text(item.to_string())}\n}"
        }
        "Image" => "Image(\"${assets/image.png}\")",
        "Icon" => "Icon(\"${settings}\")",
        "TextField" => "TextField(\"${Placeholder}\").value($${text})",
        "TextEditor" => "TextEditor(\"${Code}\").value($${text})",
        "SecureField" => "SecureField(\"${Password}\").value($${password})",
        "Checkbox" => "Checkbox(\"${Label}\").checked($${checked})",
        "Toggle" => "Toggle(\"${Enabled}\").checked($${enabled})",
        "Progress" => "Progress(${0.5})",
        "Divider" => "Divider()",
        "AdaptiveGrid" => {
            r#"AdaptiveGrid {
    ${Text("Item")}
}.min_item_width(240).spacing(16)"#
        }
        "AdaptiveLayout" => {
            r#"AdaptiveLayout {
    ${Text("Content")}
}"#
        }
        "Gap" => "Gap(${8})",
        _ => return None,
    }))
}
/// Complete Omen modifiers at a dot or partially typed name.
pub fn modifier_completions(source: &str, cursor: usize) -> Vec<DslCompletion> {
    let Some(before) = source.get(..cursor) else {
        return Vec::new();
    };
    let identifier = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let start = before.trim_end_matches(identifier).len();
    let Some(dot) = start
        .checked_sub(1)
        .filter(|&dot| source.as_bytes()[dot] == b'.')
    else {
        return Vec::new();
    };
    let prefix = &source[start..cursor];
    if prefix.starts_with(|ch: char| ch.is_ascii_digit())
        || !(source.contains("view") || source.contains("main"))
        || !is_code_at(source, dot)
    {
        return Vec::new();
    }
    let end = source.len() - source[cursor..].trim_start_matches(identifier).len();
    let has_arguments = source[end..].trim_start().starts_with(['(', '{']);
    let mut with_probe = String::with_capacity(source.len() + 20);
    with_probe.push_str(&source[..start]);
    with_probe.push_str(if has_arguments {
        "__omen_modifier_probe"
    } else {
        "__omen_modifier_probe()"
    });
    with_probe.push_str(&source[end..]);
    let node = match crate::inspector::inspect(&with_probe, dot) {
        Ok(node) => node,
        Err(_) if !has_arguments => {
            with_probe.truncate(start + "__omen_modifier_probe()".len());
            let Some(closers) = closing_delimiters(&with_probe) else {
                return Vec::new();
            };
            with_probe.push_str(&closers);
            crate::inspector::inspect(&with_probe, dot).ok().flatten()
        }
        Err(_) => None,
    };
    let Some(node) = node.filter(|node| {
        node.properties.iter().any(|property| {
            property.name == "__omen_modifier_probe"
                && property
                    .modifier_range
                    .as_ref()
                    .is_some_and(|range| range.start == dot)
        })
    }) else {
        return Vec::new();
    };
    let prefix = prefix.to_ascii_lowercase();
    let mut names = crate::modifiers::MODIFIER_NAMES
        .iter()
        .copied()
        .filter(|name| {
            !matches!(*name, "color" | "textcolor")
                && name.to_ascii_lowercase().starts_with(&prefix)
                && crate::modifiers::allowed(name, &node.component)
        })
        .collect::<Vec<_>>();
    names.sort_by_key(|name| {
        (
            !crate::inspector::MODIFIERS
                .iter()
                .any(|item| item.name == *name),
            *name,
        )
    });
    names
        .into_iter()
        .map(|name| {
            let mut snippet = Snippet::default();
            snippet.text(name);
            if !has_arguments {
                snippet.text("(");
                if let Some(arguments) =
                    crate::modifiers::completion_arguments(name, &node.component)
                {
                    snippet.slot(arguments);
                }
                snippet.text(")");
            }
            if !snippet.stops.is_empty() {
                snippet.stops.push(snippet.text.len()..snippet.text.len());
            }
            DslCompletion {
                name: format!(".{name}"),
                range: start..end,
                insert: snippet.text,
                placeholders: snippet.stops,
                additional_edits: Vec::new(),
            }
        })
        .collect()
}

fn closing_delimiters(source: &str) -> Option<String> {
    let mut delimiters = Vec::new();
    for byte in code_mask(source) {
        match byte {
            b'(' => delimiters.push(')'),
            b'[' => delimiters.push(']'),
            b'{' => delimiters.push('}'),
            b')' | b']' | b'}' if delimiters.pop()? != char::from(byte) => return None,
            _ => {}
        }
    }
    Some(delimiters.into_iter().rev().collect())
}

/// Complete all compiler components and local view properties with editable defaults.
pub fn completions(source: &str, cursor: usize) -> Vec<DslCompletion> {
    completions_with_components(source, cursor, &[])
}
/// Include reusable views discovered in other project files.
pub fn completions_with_components(
    source: &str,
    cursor: usize,
    catalog: &[DslComponent],
) -> Vec<DslCompletion> {
    let Some((range, directive, macro_start)) = context(source, cursor) else {
        return Vec::new();
    };
    let prefix = &source[range.start..cursor];
    let mut candidates = std::collections::BTreeMap::new();
    if directive {
        for (name, template) in [
            ("Design", "Design"),
            ("State", "State ${state} = ${false}"),
            ("Environment", "Environment ${app}: ${AppEnvironment}"),
            ("Responsive", "Responsive ${layout}"),
            ("Localize", "Localize ${strings} = ${AppStrings}()"),
        ] {
            candidates.insert(name.to_string(), Snippet::template(template));
        }
    } else {
        for view in catalog.iter().cloned().chain(components(source)) {
            let mut snippet = Snippet::default();
            snippet.text(&view.name);
            snippet.text("(");
            for (index, (name, value)) in view.properties.iter().enumerate() {
                if index > 0 {
                    snippet.text(", ");
                }
                snippet.text(name);
                snippet.text(": ");
                snippet.slot(value);
            }
            snippet.text(")");
            candidates.insert(view.name, snippet);
        }
        // Recover already complete declarations even while the active macro is unclosed.
        if source.parse::<TokenStream>().is_err() {
            let before = &source[..macro_start];
            let mask = code_mask(before);
            let mut closings = Vec::new();
            for ch in mask {
                match ch {
                    b'{' => closings.push('}'),
                    b'(' => closings.push(')'),
                    b'[' => closings.push(']'),
                    b'}' | b')' | b']' => {
                        closings.pop();
                    }
                    _ => {}
                }
            }
            let recovered = format!(
                "{}{}",
                before,
                closings.into_iter().rev().collect::<String>()
            );
            for view in components(&recovered) {
                let mut snippet = Snippet::default();
                snippet.text(&view.name);
                snippet.text("(");
                for (i, (name, value)) in view.properties.iter().enumerate() {
                    if i > 0 {
                        snippet.text(", ");
                    }
                    snippet.text(name);
                    snippet.text(": ");
                    snippet.slot(value);
                }
                snippet.text(")");
                candidates.insert(view.name, snippet);
            }
        }
        for name in BUILTIN_VIEW_NAMES {
            if let Some(snippet) = builtin(name) {
                candidates.insert((*name).into(), snippet);
            }
        }
    }
    let line = source[..range.start]
        .rsplit('\n')
        .next()
        .unwrap_or_default();
    let indent: String = line
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect();
    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, mut snippet)| {
            if source[range.end..].trim_start().starts_with(['(', '{']) {
                snippet = Snippet {
                    text: name.clone(),
                    stops: Vec::new(),
                };
            } else {
                for stop in &mut snippet.stops {
                    stop.start += snippet.text[..stop.start]
                        .bytes()
                        .filter(|b| *b == b'\n')
                        .count()
                        * indent.len();
                    stop.end += snippet.text[..stop.end]
                        .bytes()
                        .filter(|b| *b == b'\n')
                        .count()
                        * indent.len();
                }
                snippet.text = snippet.text.replace('\n', &format!("\n{indent}"));
                snippet.stops.push(snippet.text.len()..snippet.text.len());
            }
            DslCompletion {
                name,
                range: range.clone(),
                insert: snippet.text,
                placeholders: snippet.stops,
                additional_edits: Vec::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_builtin_inserts_valid_omen_syntax_with_default_slots() {
        let source = "view! { Home { \n    \n} }";
        let cursor = source.find("    ").unwrap() + 4;
        let items = completions(source, cursor);
        for name in BUILTIN_VIEW_NAMES {
            let item = items.iter().find(|item| item.name == *name).unwrap();
            assert!(item.insert.len() > name.len());
            let code = format!("Layout {{ {} }}", item.insert);
            crate::expand_view(code.parse().unwrap())
                .unwrap_or_else(|error| panic!("{}: {}: {error}", item.name, item.insert));
            for range in &item.placeholders {
                assert!(range.end <= item.insert.len());
                assert!(item.insert.is_char_boundary(range.start));
                assert!(item.insert.is_char_boundary(range.end));
            }
        }
    }
    #[test]
    fn modifier_completion_recovers_the_component_and_inserts_only_the_missing_suffix() {
        let source = "view! { Home { Text(\"Hello\").pa } }";
        let cursor = source.find(".pa").unwrap() + 3;
        let matches = modifier_completions(source, cursor);
        let padding = matches.iter().find(|item| item.name == ".padding").unwrap();
        assert_eq!(&source[padding.range.clone()], "pa");
        assert_eq!(padding.insert, "padding(24)");
        assert_eq!(&padding.insert[padding.placeholders[0].clone()], "24");
        assert_eq!(
            matches
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            [".padding", ".paddingToken"]
        );

        let source = "view! { Home { Layout { Text(\"Hello\") }.";
        let matches = modifier_completions(source, source.len());
        assert!(matches.iter().any(|item| item.name == ".mode"));
        assert!(matches.iter().any(|item| item.name == ".accessibilityRole"));
        assert!(!matches.iter().any(|item| item.name == ".checked"));
    }

    #[test]
    fn modifier_completion_offers_only_the_canonical_text_color() {
        let source = r#"view! { Home { Text("Hello"). } }"#;
        let cursor = source.find('.').unwrap() + 1;
        let names: Vec<_> = modifier_completions(source, cursor)
            .into_iter()
            .map(|item| item.name)
            .filter(|name| matches!(name.as_str(), ".color" | ".textColor" | ".textcolor"))
            .collect();
        assert_eq!(names, [".textColor"]);
    }

    #[test]
    fn modifier_completion_leaves_rust_members_and_existing_arguments_alone() {
        for source in [
            "fn f() { let _ = value.",
            "view! { Home { Text(value.",
            "view! { Home { Button(\"OK\") { value.",
            "view! { Home { Text(\"Hello. } }",
        ] {
            assert!(modifier_completions(source, source.find('.').unwrap() + 1).is_empty());
        }
        let source = "main! { Text(\"Title\").pad(12) }";
        let cursor = source.find(".pad").unwrap() + 4;
        let matches = modifier_completions(source, cursor);
        let padding = matches.iter().find(|item| item.name == ".padding").unwrap();
        assert_eq!(&source[padding.range.clone()], "pad");
        assert_eq!(padding.insert, "padding");
    }

    #[test]
    fn custom_view_arguments_and_unfinished_macros_get_typed_defaults() {
        let source = "view! { pub Greeting(title: String, count: usize, checked: bool) { Text(title) } }\nview! { Home { Gree";
        let item = completions(source, source.len())
            .into_iter()
            .find(|item| item.name == "Greeting")
            .unwrap();
        assert_eq!(
            item.insert,
            "Greeting(title: \"value\", count: 0usize, checked: false)"
        );
        assert_eq!(item.placeholders.len(), 4);
    }
    #[test]
    fn completion_preserves_existing_arguments_and_indents_new_blocks() {
        let source = "view! { Home {\n    But\n} }";
        let item = completions(source, source.find("But").unwrap() + 3).remove(0);
        assert_eq!(
            item.insert,
            "Button(\"Button\") {\n        // Action\n    }"
        );
        assert_eq!(&item.insert[item.placeholders[1].clone()], "// Action");
        let source = "view! { Home { Tex(\"Keep\") } }";
        let item = completions(source, source.find("Tex").unwrap() + 3).remove(0);
        assert_eq!(item.insert, "Text");
        assert!(item.placeholders.is_empty());
    }
    #[test]
    fn completion_ignores_comments_strings_raw_strings_and_outside_code() {
        for source in [
            "fn main() { Tex",
            "view! { Home { // Tex",
            "view! { Home { /* Tex */ } }",
            "view! { Home { Text(\"Tex\") } }",
            "view! { Home { let text=r#\"Tex\"#; } }",
        ] {
            let cursor = source.find("Tex").unwrap() + 3;
            // Text(...) itself is code; target the argument's Tex token.
            let cursor = if source.contains("Text(\"Tex") {
                source.find("\"Tex").unwrap() + 4
            } else {
                cursor
            };
            assert!(completions(source, cursor).is_empty(), "{source}");
        }
    }
    #[test]
    fn directives_include_state_environment_and_localization_placeholders() {
        for name in ["State", "Environment", "Localize"] {
            let source = format!("view! {{ Home {{ @{name} }} }}");
            let cursor = source.find(name).unwrap() + name.len();
            let item = completions(&source, cursor).remove(0);
            assert!(item.insert.starts_with(name));
            assert!(item.placeholders.len() >= 3);
        }
    }
}

/// Built-in view snippets for an explicit insertion library, without prefix filtering.
/// Ranges are insertion-only; the caller retains the saved editor selection.
pub fn view_library(source: &str, cursor: usize) -> Vec<DslCompletion> {
    if !source.is_char_boundary(cursor) {
        return Vec::new();
    }
    let indent: String = source[..cursor]
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    BUILTIN_VIEW_NAMES
        .iter()
        .filter_map(|name| {
            let mut snippet = builtin(name)?;
            for stop in &mut snippet.stops {
                stop.start += snippet.text[..stop.start]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    * indent.len();
                stop.end += snippet.text[..stop.end]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    * indent.len();
            }
            snippet.text = snippet.text.replace('\n', &format!("\n{indent}"));
            snippet.stops.push(snippet.text.len()..snippet.text.len());
            Some(DslCompletion {
                name: (*name).into(),
                range: cursor..cursor,
                insert: snippet.text,
                placeholders: snippet.stops,
                additional_edits: Vec::new(),
            })
        })
        .collect()
}

#[cfg(test)]
mod library_tests {
    #[test]
    fn library_inserts_complete_indented_snippets_without_replacing_a_prefix() {
        let source = "view! { Home {\n    ";
        let items = super::view_library(source, source.len());
        assert_eq!(items.len(), super::BUILTIN_VIEW_NAMES.len());
        let button = items.iter().find(|item| item.name == "Button").unwrap();
        assert_eq!(button.range, source.len()..source.len());
        assert_eq!(
            button.insert,
            "Button(\"Button\") {\n        // Action\n    }"
        );
        for item in items {
            for range in item.placeholders {
                assert!(range.start <= range.end && range.end <= item.insert.len());
                assert!(item.insert.is_char_boundary(range.start));
                assert!(item.insert.is_char_boundary(range.end));
            }
        }
        assert!(super::view_library("友", 1).is_empty());
    }
}
