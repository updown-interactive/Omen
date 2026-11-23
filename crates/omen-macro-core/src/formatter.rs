//! Source-preserving formatting for Omen's declarative view modifier chains.
use crate::ast::{Content, ModifierArg, ViewNode, ViewTree};
use proc_macro2::{TokenStream, TokenTree};
use std::{collections::BTreeMap, ops::Range, str::FromStr};

/// Put view modifiers on separate lines inside `view!` and `main!` macros.
/// Rust expressions, comments and literals are preserved byte for byte. Invalid
/// or incomplete macro bodies are left unchanged. This supplements rustfmt.
pub fn format_document(source: &str) -> String {
    let Ok(tokens) = TokenStream::from_str(source) else {
        return source.to_owned();
    };
    let mut edits = Edits::default();
    macros(tokens, source, &mut edits);
    apply(source, edits)
}

/// Format a standalone view fragment, such as a component copied from a macro.
/// Invalid DSL fragments are returned unchanged.
pub fn format_fragment(source: &str) -> String {
    let Ok(tokens) = TokenStream::from_str(source) else {
        return source.to_owned();
    };
    let mut edits = Edits::default();
    body(tokens, source, &mut edits);
    apply(source, edits)
}

#[derive(Default)]
struct Edits {
    replacements: BTreeMap<usize, (Range<usize>, String)>,
    modifiers: Vec<Range<usize>>,
}

/// Source ranges of view-modifier names inside Omen macros, excluding method
/// calls inside Rust arguments, comments and strings.
pub fn modifier_ranges(source: &str) -> Vec<Range<usize>> {
    let Ok(tokens) = TokenStream::from_str(source) else {
        return Vec::new();
    };
    let mut analysis = Edits::default();
    macros(tokens, source, &mut analysis);
    analysis.modifiers.sort_by_key(|range| range.start);
    analysis.modifiers.dedup();
    analysis.modifiers
}
fn macros(tokens: TokenStream, source: &str, edits: &mut Edits) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for window in tokens.windows(3) {
        if matches!(&window[0],TokenTree::Ident(name) if name=="view" || name=="main")
            && matches!(&window[1],TokenTree::Punct(punct) if punct.as_char()=='!')
            && let TokenTree::Group(group) = &window[2]
        {
            body(group.stream(), source, edits);
        }
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            macros(group.stream(), source, edits);
        }
    }
}
fn body(tokens: TokenStream, source: &str, edits: &mut Edits) {
    // Reuse the compiler's grammar, including state bindings and action sugar.
    let Ok(tokens) = crate::sugar::actions(tokens).and_then(crate::parser::bindings) else {
        return;
    };
    if crate::is_view_declaration(tokens.clone()) {
        if let Ok(views) = syn::parse2::<crate::ast::NamedViews>(tokens) {
            for view in views.0 {
                nodes(&view.tree, source, edits, 0);
            }
        }
    } else if let Ok(tree) = syn::parse2::<ViewTree>(tokens) {
        nodes(&tree, source, edits, 0);
    }
}
fn indentation(source: &str, offset: usize, parent: usize) -> usize {
    let prefix = &source[..offset];
    let line = prefix.rsplit('\n').next().unwrap_or_default();
    if line.chars().all(|ch| ch == ' ' || ch == '\t') {
        line.chars().map(|ch| if ch == '\t' { 4 } else { 1 }).sum()
    } else {
        parent
    }
}
fn nodes(tree: &ViewTree, source: &str, edits: &mut Edits, parent: usize) {
    visit(&tree.0, source, edits, parent);
}
fn visit(nodes: &[ViewNode], source: &str, edits: &mut Edits, parent: usize) {
    for node in nodes {
        match node {
            ViewNode::Element(element) => {
                let base = indentation(source, element.name.span().byte_range().start, parent);
                let multiline_arguments = if let Some(paren_span) = element.paren_span {
                    let open_end = paren_span.open().byte_range().end;
                    let close_start = paren_span.close().byte_range().start;
                    if open_end <= close_start && close_start <= source.len() {
                        element.arguments.len() > 1 || source[open_end..close_start].contains('\n')
                    } else {
                        false
                    }
                } else {
                    false
                };

                if multiline_arguments && let Some(paren_span) = element.paren_span {
                    let arg_indent = base + 4;
                    let open_end = paren_span.open().byte_range().end;
                    let close_start = paren_span.close().byte_range().start;

                    if !element.arguments.is_empty() {
                        let first_start = element.arguments[0].start_offset();
                        if first_start >= open_end && first_start <= source.len() {
                            let start = (source[..first_start]
                                .trim_end_matches(char::is_whitespace)
                                .len())
                            .max(open_end);
                            let replacement = format!("\n{}", " ".repeat(arg_indent));
                            if source[start..first_start].replace("\r\n", "\n") != replacement {
                                edits
                                    .replacements
                                    .insert(first_start, (start..first_start, replacement));
                            }
                        }

                        for i in 0..element.arguments.len().saturating_sub(1) {
                            let prev_end = element.arguments[i].end_offset();
                            let next_start = element.arguments[i + 1].start_offset();
                            if prev_end <= next_start && next_start <= source.len() {
                                let slice = &source[prev_end..next_start];
                                if let Some(comma_pos) = slice.find(',') {
                                    let comma_end = prev_end + comma_pos + 1;
                                    let start = (source[..next_start]
                                        .trim_end_matches(char::is_whitespace)
                                        .len())
                                    .max(comma_end);
                                    let replacement = format!("\n{}", " ".repeat(arg_indent));
                                    if source[start..next_start].replace("\r\n", "\n")
                                        != replacement
                                    {
                                        edits
                                            .replacements
                                            .insert(next_start, (start..next_start, replacement));
                                    }
                                }
                            }
                        }

                        let last = element.arguments.last().unwrap();
                        let last_end = last.end_offset();
                        if last_end <= close_start && close_start <= source.len() {
                            let slice = &source[last_end..close_start];
                            let comma_end = slice
                                .find(',')
                                .map(|pos| last_end + pos + 1)
                                .unwrap_or(last_end);
                            let start = (source[..close_start]
                                .trim_end_matches(char::is_whitespace)
                                .len())
                            .max(comma_end);
                            let replacement = format!("\n{}", " ".repeat(base));
                            if source[start..close_start].replace("\r\n", "\n") != replacement {
                                edits
                                    .replacements
                                    .insert(close_start, (start..close_start, replacement));
                            }
                        }
                    }
                }

                let container = !matches!(element.content, Content::None) || multiline_arguments;
                let indent = base + if container { 0 } else { 4 };
                for (index, modifier) in element.modifiers.iter().enumerate() {
                    let dot = modifier.span.byte_range().start;
                    if source.as_bytes().get(dot) != Some(&b'.') {
                        continue;
                    }
                    edits.modifiers.push(modifier.name.span().byte_range());
                    // Keep the compact trailing fill helper, matching the DSL's
                    // existing convention; all other modifiers get their own line.
                    if index > 0
                        && index + 1 == element.modifiers.len()
                        && modifier.name == "fill"
                        && modifier.arguments.is_empty()
                    {
                        continue;
                    }
                    let start = source[..dot].trim_end_matches(char::is_whitespace).len();
                    let replacement = format!("\n{}", " ".repeat(indent));
                    if source[start..dot].replace("\r\n", "\n") != replacement {
                        edits.replacements.insert(dot, (start..dot, replacement));
                    }
                    for arg in &modifier.arguments {
                        if let ModifierArg::Views(tree) = arg {
                            self::nodes(tree, source, edits, base + 4);
                        }
                    }
                }
                match &element.content {
                    Content::Children(children) | Content::List { body: children, .. } => {
                        visit(children, source, edits, base + 4)
                    }
                    _ => {}
                }
            }
            ViewNode::Conditional(conditional) => {
                visit(&conditional.then_branch, source, edits, parent + 4);
                visit(&conditional.else_branch, source, edits, parent + 4);
            }
            ViewNode::Loop(loop_node) => visit(&loop_node.body, source, edits, parent + 4),
        }
    }
}
fn apply(source: &str, edits: Edits) -> String {
    let mut output = source.to_owned();
    let crlf = source.contains("\r\n");
    for (_, (range, replacement)) in edits.replacements.into_iter().rev() {
        let replacement = if crlf {
            replacement.replace('\n', "\r\n")
        } else {
            replacement
        };
        output.replace_range(range, &replacement);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_multiline_component_arguments_and_modifiers() {
        let source = r#"ProjectField(title: "Project name".to_string(), placeholder: "my-app".to_string(), value: app.project_name_input()).fill()"#;
        let expected = "ProjectField(\n    title: \"Project name\".to_string(),\n    placeholder: \"my-app\".to_string(),\n    value: app.project_name_input()\n)\n.fill()";
        let formatted = format_fragment(source);
        assert_eq!(formatted, expected);
        assert_eq!(format_fragment(&formatted), formatted);

        // Nested layout with indent and trailing comma
        let nested = "Layout {\n    ProjectField(title: \"Name\", placeholder: \"foo\", value: val,).fill()\n}";
        let expected_nested = "Layout {\n    ProjectField(\n        title: \"Name\",\n        placeholder: \"foo\",\n        value: val,\n    )\n    .fill()\n}";
        let formatted_nested = format_fragment(nested);
        assert_eq!(formatted_nested, expected_nested);
        assert_eq!(format_fragment(&formatted_nested), formatted_nested);
    }
    #[test]
    fn formats_the_parent_folder_example_and_is_idempotent() {
        let source = "Layout {\n    TextField(\"Parent folder\").value(app.project_parent.clone()).fontSize(13)\n        .frame(.infinity, 38).background(colors.surface).cornerRadius(7)\n}.padding(1).background(colors.outline).cornerRadius(8).fill()";
        let expected = "Layout {\n    TextField(\"Parent folder\")\n        .value(app.project_parent.clone())\n        .fontSize(13)\n        .frame(.infinity, 38)\n        .background(colors.surface)\n        .cornerRadius(7)\n}\n.padding(1)\n.background(colors.outline)\n.cornerRadius(8).fill()";
        let formatted = format_fragment(source);
        assert_eq!(formatted, expected);
        assert_eq!(format_fragment(&formatted), formatted);
    }
    #[test]
    fn only_formats_view_modifiers_preserving_rust_strings_and_comments() {
        let source = r##"fn f() { let x = foo().bar().baz(); let _ = view! { Layout {
    // leave this comment
    Text(r#"x.foo().bar()"#).value(app.name.clone()).fontSize(13) // keep trailing comment
        .color(.red)
}.padding(4) }; }"##;
        let formatted = format_document(source);
        assert!(formatted.contains("foo().bar().baz()"));
        assert!(formatted.contains("app.name.clone()"));
        assert!(formatted.contains("// keep trailing comment\n        .color(.red)"));
        assert!(formatted.contains("r#\"x.foo().bar()\"#"));
        assert_eq!(format_document(&formatted), formatted);
        assert_eq!(format_document("view! { Layout {"), "view! { Layout {");
    }
    #[test]
    fn supports_named_views_main_unicode_crlf_and_preserves_tokens() {
        for source in [
            "// café 👩‍💻\nview! { pub Sample { @State name: String = String::new(); Layout { Text(name.clone()).fontSize(13).color(.red) }.padding(4) } }",
            "main! { Studio().environment(env.clone()).padding(4) }",
            "view! { Layout {\r\n    Text(\"café\").fontSize(13).color(.red)\r\n}.padding(4) }",
        ] {
            let formatted = format_document(source);
            assert_ne!(formatted, source);
            assert_eq!(
                TokenStream::from_str(&formatted).unwrap().to_string(),
                TokenStream::from_str(source).unwrap().to_string()
            );
            assert_eq!(format_document(&formatted), formatted);
            if source.contains("\r\n") {
                assert!(!formatted.replace("\r\n", "").contains('\n'));
            }
        }
    }
    #[test]
    fn modifier_highlights_exclude_rust_methods_strings_and_comments() {
        let source = r#"fn f() { let name = app.name.clone(); let _ = view! { Layout {
            Text(".background(.red)").value(name.clone()).fontSize(13)
            // .color(.red)
        }.padding(4).fill() }; }"#;
        let names: Vec<_> = modifier_ranges(source)
            .into_iter()
            .map(|range| &source[range])
            .collect();
        assert_eq!(names, ["value", "fontSize", "padding", "fill"]);
    }
}
