//! Render the Markdown subset emitted by Rust and Omen hover providers.
use super::*;
use gpui::Div;

enum Block {
    Heading(usize, String),
    Prose(String),
    Code(String, String),
    Rule,
}

fn blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(language) = line
            .strip_prefix("```")
            .or_else(|| line.strip_prefix("~~~"))
        {
            let delimiter = &line[..3];
            let mut code = Vec::new();
            for line in lines.by_ref() {
                if line.trim_start().starts_with(delimiter) {
                    break;
                }
                code.push(line);
            }
            blocks.push(Block::Code(language.trim().to_owned(), code.join("\n")));
        } else if matches!(line, "---" | "***" | "___") {
            blocks.push(Block::Rule);
        } else if let Some((depth, heading)) = heading(line) {
            blocks.push(Block::Heading(depth, heading.to_owned()));
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            blocks.push(Block::Prose(format!("•  {item}")));
        } else {
            let mut paragraph = line.to_owned();
            while let Some(next) = lines.peek().map(|next| next.trim())
                && !next.is_empty()
                && !["```", "~~~", "- ", "* "]
                    .iter()
                    .any(|prefix| next.starts_with(prefix))
                && !matches!(next, "---" | "***" | "___")
                && heading(next).is_none()
            {
                paragraph.push(' ');
                paragraph.push_str(next);
                lines.next();
            }
            blocks.push(Block::Prose(paragraph));
        }
    }
    blocks
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let depth = line.bytes().take_while(|&byte| byte == b'#').count();
    (1..=6).contains(&depth).then_some(())?;
    line[depth..].strip_prefix(' ').map(|text| (depth, text))
}

fn prose(source: &str, accent: gpui::Hsla) -> gpui::StyledText {
    let mut text = String::new();
    let mut styles = Vec::new();
    let mut parts = source.split_inclusive('`');
    while let Some(part) = parts.next() {
        let Some(prefix) = part.strip_suffix('`') else {
            text.push_str(part);
            break;
        };
        text.push_str(prefix);
        let Some(code) = parts.next() else {
            text.push('`');
            break;
        };
        if let Some(code) = code.strip_suffix('`') {
            let start = text.len();
            text.push_str(code);
            styles.push((
                start..text.len(),
                gpui::HighlightStyle {
                    color: Some(accent),
                    ..Default::default()
                },
            ));
        } else {
            text.push('`');
            text.push_str(code);
        }
    }
    gpui::StyledText::new(text).with_highlights(styles)
}

pub(super) fn content(
    markdown: &str,
    diagnostics: &[&omen_core::EditorDiagnostic],
    dark: bool,
    accent: gpui::Hsla,
) -> Div {
    let border = gpui::rgb(if dark { 0x3e4550 } else { 0xdce2eb });
    let code_background = gpui::rgb(if dark { 0x1a1f28 } else { 0xf1f4f9 });
    let mut content = div().flex().flex_col().gap(px(8.)).p(px(12.));
    for block in blocks(markdown) {
        let row = match block {
            Block::Heading(depth, heading) => div()
                .text_size(px(if depth <= 2 { 15. } else { 12. }))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(prose(&heading, accent)),
            Block::Prose(paragraph) => div().line_height(px(18.)).child(prose(&paragraph, accent)),
            Block::Code(language, code) => {
                let language = language.split([',', ' ']).next().unwrap_or_default();
                let code = if matches!(language, "" | "rust" | "rs" | "omen") {
                    syntax::highlighted_code(&code, dark)
                } else {
                    gpui::StyledText::new(code)
                };
                div()
                    .p(px(9.))
                    .min_w_0()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(border)
                    .bg(code_background)
                    .font_family("monospace")
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .child(code)
            }
            Block::Rule => div().h(px(1.)).bg(border),
        };
        content = content.child(row);
    }
    for diagnostic in diagnostics {
        let color = gpui::rgb(if diagnostic.error {
            if dark { 0xf5a5ab } else { 0xa12835 }
        } else if dark {
            0xe6c17f
        } else {
            0x8d540a
        });
        content = content.child(
            div()
                .pt(px(7.))
                .border_t_1()
                .border_color(border)
                .text_size(px(11.))
                .text_color(color)
                .child(diagnostic.message.clone()),
        );
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_keeps_signature_examples_and_indentation() {
        let markdown = "```rust\nfn count() -> u32\n```\n---\n## Examples\n\nUse `count()`.\n\n```rust,ignore\nfn main() {\n    // Read count\n    let n = count();\n}\n```";
        let parsed = blocks(markdown);
        assert!(matches!(&parsed[..], [
            Block::Code(signature, _), Block::Rule, Block::Heading(2, heading),
            Block::Prose(prose), Block::Code(language, example)
        ] if signature == "rust" && heading == "Examples" && prose == "Use `count()`."
            && language == "rust,ignore" && example.contains("\n    let n = count();")));
    }
}
