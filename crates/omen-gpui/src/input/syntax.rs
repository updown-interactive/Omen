//! Fast visible-line fallback highlighting, supplemented by versioned LSP tokens.
use super::*;
use omen_core::{SyntaxKind, SyntaxSpan};

pub(super) struct SyntaxEdit {
    pub range: Range<usize>,
    pub inserted: usize,
    pub touching: bool,
}

pub(super) fn identifier_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

pub(super) fn rebase_highlights(spans: &mut Vec<SyntaxSpan>, edits: &[SyntaxEdit]) {
    for edit in edits {
        spans.retain_mut(|span| {
            let range = &edit.range;
            if span.range.end < range.start || (!edit.touching && span.range.end == range.start) {
                return true;
            }
            if span.range.start > range.end || (!edit.touching && span.range.start == range.end) {
                let removed = range.len();
                span.range.start = span.range.start - removed + edit.inserted;
                span.range.end = span.range.end - removed + edit.inserted;
                return true;
            }
            false
        });
    }
}

pub(super) fn highlighted_code(code: &str, dark: bool) -> gpui::StyledText {
    let mut spans = Vec::<SyntaxSpan>::new();
    for span in lexical(code) {
        if spans
            .last()
            .is_some_and(|previous| previous.range.end > span.range.start)
            && let Some(previous) = spans.pop()
        {
            if previous.range.start < span.range.start {
                spans.push(SyntaxSpan {
                    range: previous.range.start..span.range.start,
                    kind: previous.kind,
                });
            }
            let end = span.range.end;
            spans.push(span);
            if previous.range.end > end {
                spans.push(SyntaxSpan {
                    range: end..previous.range.end,
                    kind: previous.kind,
                });
            }
        } else {
            spans.push(span);
        }
    }
    gpui::StyledText::new(code.to_owned()).with_highlights(spans.into_iter().map(|span| {
        (
            span.range,
            gpui::HighlightStyle {
                color: Some(color(span.kind, dark)),
                ..Default::default()
            },
        )
    }))
}

fn lexical(text: &str) -> Vec<SyntaxSpan> {
    let mut spans = Vec::new();
    let mut index = 0;
    let mut previous = "";
    let mut attribute = false;
    while index < text.len() {
        let start = index;
        let rest = &text[start..];
        let ch = rest.chars().next().unwrap();
        index += ch.len_utf8();
        let mut kind = None;
        if rest.starts_with("//") {
            index = rest.find('\n').map_or(text.len(), |end| start + end);
            kind = Some(SyntaxKind::Comment);
        } else if rest.starts_with("/*") {
            let mut depth = 1;
            index = start + 2;
            while index < text.len() && depth > 0 {
                if text[index..].starts_with("/*") {
                    depth += 1;
                    index += 2;
                } else if text[index..].starts_with("*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += text[index..].chars().next().unwrap().len_utf8();
                }
            }
            kind = Some(SyntaxKind::Comment);
        } else if rest.starts_with("r#")
            && rest[2..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        {
            index = start + 2;
            while index < text.len()
                && text[index..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            {
                index += text[index..].chars().next().unwrap().len_utf8();
            }
            kind = Some(SyntaxKind::Variable);
        } else if ch == 'r' && rest[1..].trim_start_matches('#').starts_with('"') {
            let hashes = rest[1..].bytes().take_while(|byte| *byte == b'#').count();
            let body = start + 2 + hashes;
            let terminator = format!("\"{}", "#".repeat(hashes));
            index = text[body..]
                .find(&terminator)
                .map_or(text.len(), |at| body + at + terminator.len());
            kind = Some(SyntaxKind::String);
        } else if ch == '"'
            || ch == '\''
                && rest[1..].chars().next().is_some_and(|next| {
                    next == '\\' || rest[1 + next.len_utf8()..].starts_with('\'')
                })
        {
            let mut escaped = false;
            while index < text.len() {
                let next = text[index..].chars().next().unwrap();
                index += next.len_utf8();
                if next == ch && !escaped {
                    break;
                }
                escaped = next == '\\' && !escaped;
            }
            spans.push(SyntaxSpan {
                range: start..index,
                kind: SyntaxKind::String,
            });
            let mut escape = start + ch.len_utf8();
            while escape < index {
                let current = text[escape..].chars().next().unwrap();
                if current == '\\' {
                    let end = (escape + 1..index)
                        .find(|at| text.is_char_boundary(*at))
                        .unwrap_or(index);
                    let end = if end < index {
                        end + text[end..].chars().next().unwrap().len_utf8()
                    } else {
                        end
                    };
                    spans.push(SyntaxSpan {
                        range: escape..end,
                        kind: SyntaxKind::Escape,
                    });
                    escape = end;
                } else {
                    escape += current.len_utf8();
                }
            }
            continue;
        } else if ch == '\''
            && rest[1..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        {
            while index < text.len()
                && text[index..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            {
                index += text[index..].chars().next().unwrap().len_utf8();
            }
            kind = Some(SyntaxKind::Lifetime);
        } else if ch.is_ascii_digit() {
            while index < text.len() {
                let next = text[index..].chars().next().unwrap();
                if next.is_ascii_alphanumeric()
                    || next == '_'
                    || next == '.'
                        && !text[index..].starts_with("..")
                        && text[index + 1..].starts_with(|ch: char| ch.is_ascii_digit())
                {
                    index += next.len_utf8();
                } else {
                    break;
                }
            }
            kind = Some(SyntaxKind::Number);
        } else if ch.is_alphabetic() || ch == '_' || ch == '@' {
            while index < text.len()
                && text[index..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            {
                index += text[index..].chars().next().unwrap().len_utf8();
            }
            let word = &text[start..index];
            let following = text[index..].trim_start();
            kind = Some(
                if [
                    "if", "else", "match", "for", "while", "loop", "in", "return", "break",
                    "continue", "yield", "await", "try",
                ]
                .contains(&word)
                {
                    SyntaxKind::ControlFlow
                } else if ["true", "false"].contains(&word) {
                    SyntaxKind::Boolean
                } else if [
                    "fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "use", "mod",
                    "self", "Self", "super", "crate", "const", "static", "async", "move", "where",
                    "type", "unsafe", "extern", "as", "ref", "dyn", "union", "default", "abstract",
                    "become", "box", "do", "final", "macro", "override", "priv", "typeof",
                    "unsized", "virtual",
                ]
                .contains(&word)
                {
                    SyntaxKind::Keyword
                } else if attribute {
                    SyntaxKind::Attribute
                } else if ch == '@' || following.starts_with('!') && !following.starts_with("!=") {
                    SyntaxKind::Macro
                } else if previous == "fn" || following.starts_with('(') {
                    SyntaxKind::Function
                } else if previous == "mod" || following.starts_with("::") && !ch.is_uppercase() {
                    SyntaxKind::Namespace
                } else if word.chars().any(char::is_alphabetic)
                    && word.chars().all(|ch| !ch.is_lowercase())
                {
                    SyntaxKind::Constant
                } else if ch.is_uppercase()
                    || [
                        "bool", "char", "str", "usize", "isize", "u8", "u16", "u32", "u64", "u128",
                        "i8", "i16", "i32", "i64", "i128", "f32", "f64",
                    ]
                    .contains(&word)
                {
                    SyntaxKind::Type
                } else if text[..start].trim_end().ends_with('.') {
                    SyntaxKind::Property
                } else {
                    SyntaxKind::Variable
                },
            );
            previous = word;
        } else if "+-*/%=!&|^<>?".contains(ch) {
            kind = Some(SyntaxKind::Operator);
        } else if "()[]{};:,.#".contains(ch) {
            if ch == '#' {
                attribute = true;
            }
            if ch == ']' {
                attribute = false;
            }
            kind = Some(SyntaxKind::Punctuation);
        }
        if let Some(kind) = kind {
            spans.push(SyntaxSpan {
                range: start..index,
                kind,
            });
        }
    }
    spans
}
fn color(kind: SyntaxKind, dark: bool) -> gpui::Hsla {
    // Coordinated palettes: violet declarations, rose control flow, blue functions,
    // teal types, gold constants, peach parameters and muted slate comments.
    let (dark_color, light_color) = match kind {
        SyntaxKind::Keyword => (0xcba6f7, 0x8839af),
        SyntaxKind::ControlFlow => (0xf5a9cf, 0xa52a75),
        SyntaxKind::String => (0xa6d189, 0x2e702b),
        SyntaxKind::Escape => (0xf4bf75, 0x9a5300),
        SyntaxKind::Comment => (0x8291a8, 0x64748b),
        SyntaxKind::Number => (0xf5b78d, 0xa34b12),
        SyntaxKind::Boolean => (0xf5b78d, 0x9f391d),
        SyntaxKind::Type => (0x81d4c0, 0x08786a),
        SyntaxKind::Function => (0x89baff, 0x1259b2),
        SyntaxKind::ViewModifier => (0xf2c078, 0x9a570c),
        SyntaxKind::Macro => (0xc4a7ff, 0x7950b3),
        SyntaxKind::Attribute => (0xdac28c, 0x87651c),
        SyntaxKind::Namespace => (0x9eb9e8, 0x405f9d),
        SyntaxKind::Variable => (0xd6deeb, 0x28364d),
        SyntaxKind::Parameter => (0xf2c5a5, 0x90502c),
        SyntaxKind::Property => (0x9cd6f5, 0x096f91),
        SyntaxKind::Constant => (0xeccc87, 0x8a5b00),
        SyntaxKind::Lifetime => (0xf0a6bd, 0xa63666),
        SyntaxKind::Operator => (0x91d7e3, 0x167384),
        SyntaxKind::Punctuation => (0xa5afc4, 0x697286),
    };
    gpui::rgb(if dark { dark_color } else { light_color }).into()
}
pub(super) fn runs(
    input: &TextInput,
    text: &str,
    start: usize,
    base: TextRun,
    brackets: Option<(usize, usize)>,
) -> Vec<TextRun> {
    let Some(document) = &input.code_document else {
        return vec![base];
    };
    let state = document.state.get();
    let dark = base.color.l > 0.5;
    let mut spans = lexical(text);
    let local_count = spans.len();
    if state.text.same_snapshot(&input.content)
        && state
            .highlight_identity
            .is_none_or(|identity| identity == input.content.identity())
    {
        let first = state
            .highlights
            .partition_point(|span| span.range.end <= start);
        for span in state.highlights[first..]
            .iter()
            .take_while(|span| span.range.start < start + text.len())
        {
            spans.push(SyntaxSpan {
                range: span.range.start.saturating_sub(start)
                    ..span.range.end.saturating_sub(start).min(text.len()),
                kind: span.kind,
            });
        }
    }
    let mut boundaries = vec![0, text.len()];
    for offset in brackets.into_iter().flat_map(|(a, b)| [a, b]) {
        if offset >= start && offset < start + text.len() {
            boundaries.extend([offset - start, offset - start + 1]);
        }
    }
    for span in &spans {
        boundaries.extend([span.range.start, span.range.end]);
    }
    for diagnostic in state.diagnostics.iter().filter(|diagnostic| {
        diagnostic.range.start < start + text.len() && diagnostic.range.end > start
    }) {
        boundaries.extend([
            diagnostic.range.start.saturating_sub(start),
            diagnostic.range.end.saturating_sub(start).min(text.len()),
        ]);
    }
    boundaries.retain(|offset| *offset <= text.len() && text.is_char_boundary(*offset));
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter(|pair| pair[0] < pair[1])
        .map(|pair| {
            let offset = pair[0];
            let local = spans[..local_count]
                .iter()
                .rev()
                .find(|span| span.range.contains(&offset));
            let semantic = || {
                spans[local_count..]
                    .iter()
                    .rev()
                    .find(|span| span.range.contains(&offset))
            };
            let token = if local.is_some_and(|span| {
                matches!(
                    span.kind,
                    SyntaxKind::String | SyntaxKind::Escape | SyntaxKind::Comment
                )
            }) {
                local
            } else {
                semantic().or(local)
            };
            let kind = token.map(|token| token.kind);
            let diagnostic = state.diagnostics.iter().find(|diagnostic| {
                diagnostic.range.start <= start + offset && diagnostic.range.end > start + offset
            });
            TextRun {
                background_color: if brackets
                    .is_some_and(|(a, b)| start + offset == a || start + offset == b)
                {
                    Some(input.accent.opacity(0.22))
                } else {
                    None
                },
                len: pair[1] - offset,
                color: kind.map_or(base.color, |kind| color(kind, dark)),
                underline: diagnostic.map(|diagnostic| UnderlineStyle {
                    color: Some(if diagnostic.error {
                        gpui::rgb(0xe06c75).into()
                    } else {
                        gpui::rgb(0xd9a441).into()
                    }),
                    thickness: px(1.),
                    wavy: true,
                }),
                ..base.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rust_tokens_distinguish_literals_lifetimes_calls_and_annotations() {
        let source = r###"#[derive(Debug)] fn café<'a>(value: &'a str) -> bool { if true { println!("hi\n"); let r#type = r##"match \" raw"##; } }"###;
        let spans = lexical(source);
        let has = |word: &str, kind| {
            spans
                .iter()
                .any(|span| &source[span.range.clone()] == word && span.kind == kind)
        };
        assert!(has("derive", SyntaxKind::Attribute));
        assert!(has("café", SyntaxKind::Function));
        assert!(has("'a", SyntaxKind::Lifetime));
        assert!(has("str", SyntaxKind::Type));
        assert!(has("if", SyntaxKind::ControlFlow));
        assert!(has("true", SyntaxKind::Boolean));
        assert!(has("println", SyntaxKind::Macro));
        assert!(has("r#type", SyntaxKind::Variable));
        assert!(has("\\n", SyntaxKind::Escape));
        assert!(
            spans.iter().any(|span| span.kind == SyntaxKind::String
                && source[span.range.clone()].starts_with("r##"))
        );
        assert!(!has("match", SyntaxKind::ControlFlow));
        for span in spans {
            assert!(
                source.is_char_boundary(span.range.start)
                    && source.is_char_boundary(span.range.end)
            );
        }
    }
    #[test]
    fn nested_comments_and_unicode_character_literals_remain_whole() {
        let source = "/* outer /* inner */ end */ let letter = 'é'; let count = 0xff_u8; // tail";
        let spans = lexical(source);
        assert_eq!(
            &source[spans[0].range.clone()],
            "/* outer /* inner */ end */"
        );
        assert!(spans.iter().any(|span|span.kind==SyntaxKind::String && &source[span.range.clone()]=="'é'"));
        assert!(spans.iter().any(
            |span| span.kind == SyntaxKind::Number && &source[span.range.clone()] == "0xff_u8"
        ));
        assert_eq!(spans.last().unwrap().kind, SyntaxKind::Comment);
    }
}
