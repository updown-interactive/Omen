use super::*;
use syn::UseTree;

pub(super) fn completions(source: &str, cursor: usize) -> Vec<DslCompletion> {
    if cursor > source.len() || !source.is_char_boundary(cursor) {
        return Vec::new();
    }
    let start = cursor
        - source[..cursor]
            .chars()
            .rev()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
    let end = cursor
        + source[cursor..]
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
    let mask = code_mask(source);
    if mask[start..cursor] != source.as_bytes()[start..cursor]
        || cursor < mask.len() && mask[cursor] == 0
    {
        return Vec::new();
    }
    let line_start = source[..start].rfind('\n').map_or(0, |n| n + 1);
    if !source[line_start..start].trim().is_empty() || source[end..].trim_start().starts_with('!') {
        return Vec::new();
    }
    let mut stack = Vec::new();
    for (offset, byte) in mask[..start].iter().enumerate() {
        match byte {
            b'{' | b'(' | b'[' => stack.push((*byte, offset)),
            b'}' | b')' | b']' => {
                stack.pop();
            }
            _ => {}
        }
    }
    for (kind, offset) in &stack {
        if *kind != b'{' {
            return Vec::new();
        }
        let before = String::from_utf8_lossy(&mask[..*offset]).replace('\0', " ");
        let mut words = before.split_whitespace().rev();
        let _name = words.next();
        if words.next() != Some("mod") {
            return Vec::new();
        }
    }
    let scope_start = stack.last().map_or(0, |(_, offset)| offset + 1);
    let scope_end = if let Some((_, open)) = stack.last() {
        let mut depth = 1;
        let mut end = source.len();
        for (index, byte) in mask[*open + 1..].iter().enumerate() {
            if *byte == b'{' {
                depth += 1;
            } else if *byte == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = *open + 1 + index;
                    break;
                }
            }
        }
        end
    } else {
        source.len()
    };
    let mut import_present = false;
    let mut depth = 0_i32;
    for index in scope_start..scope_end {
        if depth == 0
            && mask.get(index..index + 3) == Some(b"use")
            && (index == 0
                || !(source.as_bytes()[index - 1].is_ascii_alphanumeric()
                    || source.as_bytes()[index - 1] == b'_'))
            && let Some(end) = mask[index..scope_end].iter().position(|byte| *byte == b';')
            && let Ok(item) = syn::parse_str::<syn::ItemUse>(&source[index..index + end + 1])
        {
            import_present |= prelude(&item.tree, &mut Vec::new());
        }
        match mask[index] {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => depth -= 1,
            _ => {}
        }
    }
    let mut import_offset = header_end(source, scope_start, start);
    if import_offset == start {
        import_offset = line_start.max(scope_start);
    }
    let edits = if import_present {
        Vec::new()
    } else {
        let indent = &source[line_start..start];
        vec![CompletionEdit {
            range: import_offset..import_offset,
            insert: format!("{indent}use omen::prelude::*;\n\n"),
        }]
    };
    let prefix = &source[start..cursor];
    [("view", "view! {\n    pub ${MyView} {\n        Layout {\n            ${Text(\"Hello, Omen!\")}\n        }\n    }\n}"),
     ("state", "state! {\n    pub ${MyState} {\n        ${count: i32 = 0;}\n    }\n}"),
     ("environment", "environment! {\n    pub ${AppEnvironment} {\n        ${@Reactive count: i32 = 0;}\n    }\n}"),
     ("locale", "locale! {\n    pub ${AppStrings} {\n        default: \"${en}\"\n        ${en} {\n            ${welcome}: \"${Welcome}\"\n        }\n    }\n}"),
     ("main", "main! {\n    ${MyView}()\n}")].into_iter().filter(|(name,_)|name.starts_with(prefix)).map(|(name,template)|{
        let mut snippet=Snippet::template(template);let indent=&source[line_start..start];
        for stop in &mut snippet.stops {stop.start+=snippet.text[..stop.start].bytes().filter(|b|*b==b'\n').count()*indent.len();stop.end+=snippet.text[..stop.end].bytes().filter(|b|*b==b'\n').count()*indent.len();}
        snippet.text=snippet.text.replace('\n',&format!("\n{indent}"));snippet.stops.push(snippet.text.len()..snippet.text.len());
        DslCompletion {name:format!("{name}!"),range:start..end,insert:snippet.text,placeholders:snippet.stops,additional_edits:edits.clone()}
    }).collect()
}
fn prelude(tree: &UseTree, path: &mut Vec<String>) -> bool {
    match tree {
        UseTree::Path(item) => {
            path.push(item.ident.to_string());
            let found = prelude(&item.tree, path);
            path.pop();
            found
        }
        UseTree::Group(group) => group.items.iter().any(|item| prelude(item, path)),
        UseTree::Glob(_) => path == &["omen"] || path == &["omen", "prelude"],
        _ => false,
    }
}
fn header_end(source: &str, start: usize, limit: usize) -> usize {
    let mut offset = start;
    if start == 0 && source.starts_with("#!") && !source.starts_with("#![") {
        offset = source.find('\n').map_or(source.len(), |n| n + 1);
    }
    loop {
        let rest = &source[offset..limit];
        let trim = rest.trim_start_matches(char::is_whitespace);
        offset += rest.len() - trim.len();
        if trim.starts_with("//") {
            offset += trim.find('\n').map_or(trim.len(), |n| n + 1);
        } else if trim.starts_with("/*") {
            let mask = code_mask(trim);
            let length = mask
                .iter()
                .zip(trim.bytes())
                .position(|(a, b)| *a == b && b != b'\n')
                .unwrap_or(trim.len());
            if length == 0 {
                break;
            }
            offset += length;
        } else if trim.starts_with("#![") {
            if let Ok(tokens) = trim.parse::<TokenStream>() {
                let tokens: Vec<_> = tokens.into_iter().collect();
                if let Some(TokenTree::Group(group)) = tokens.get(2) {
                    offset += group.span().byte_range().end;
                    continue;
                }
            }
            let Some(end) = trim.find(']') else {
                break;
            };
            offset += end + 1;
        } else {
            break;
        }
        if offset >= limit {
            return limit;
        }
    }
    offset.min(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(source: &str, item: &DslCompletion) -> String {
        let mut text = source.to_string();
        let mut edits: Vec<_> = item
            .additional_edits
            .iter()
            .map(|edit| (edit.range.clone(), edit.insert.clone()))
            .collect();
        edits.push((item.range.clone(), item.insert.clone()));
        edits.sort_by_key(|(range, _)| (range.start, range.end));
        for (range, value) in edits.into_iter().rev() {
            text.replace_range(range, &value);
        }
        text
    }
    #[test]
    fn shortcuts_insert_one_prelude_and_valid_declarations() {
        for name in ["view", "state", "environment", "locale", "main"] {
            let item = completions(name, name.len()).remove(0);
            let source = apply(name, &item);
            assert_eq!(source.matches("use omen::prelude::*;").count(), 1);
            syn::parse_file(&source).unwrap();
            assert!(item.placeholders.len() >= 2);
        }
    }
    #[test]
    fn imports_are_detected_direct_grouped_or_later_and_comments_do_not_count() {
        for import in [
            "use omen::prelude::*;",
            "use ::omen::prelude :: *;",
            "use omen::{prelude::*, Text};",
            "use omen::*;",
        ] {
            for source in [format!("{import}\nview"), format!("view\n{import}")] {
                let cursor = source.find("view").unwrap() + 4;
                assert!(
                    completions(&source, cursor)[0].additional_edits.is_empty(),
                    "{source}"
                );
            }
        }
        let source = "// use omen::prelude::*;\nview";
        assert_eq!(
            completions(source, source.len())[0].additional_edits.len(),
            1
        );
    }
    #[test]
    fn preserves_crate_headers_and_places_nested_imports_in_their_own_scope() {
        let source = "#!/usr/bin/env omen\n//! Documentation\n#![allow(unused)]\n\nview";
        let result = apply(source, &completions(source, source.len())[0]);
        assert!(result.starts_with("#!/usr/bin/env omen\n//! Documentation\n#![allow(unused)]"));
        syn::parse_file(&result).unwrap();
        let source = "use omen::prelude::*;\nmod inner {\n    state\n}";
        let cursor = source.find("state").unwrap() + 5;
        let result = apply(source, &completions(source, cursor)[0]);
        assert_eq!(result.matches("use omen::prelude::*;").count(), 2);
        syn::parse_file(&result).unwrap();
    }
    #[test]
    fn does_not_expand_declarations_in_strings_comments_functions_or_existing_macros() {
        for source in [
            "// view",
            "const S:&str=\"view",
            "fn f(){\nview",
            "view! { Home { view",
            "use omen::view",
        ] {
            assert!(completions(source, source.len()).is_empty(), "{source}");
        }
        assert!(completions("view! {}", 4).is_empty());
    }
}
