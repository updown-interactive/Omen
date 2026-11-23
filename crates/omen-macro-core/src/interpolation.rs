//! Compile Text interpolation into formatting expressions before state rewriting.
use crate::ast::*;
use syn::{Expr, Lit, LitStr};

fn expression(text: &LitStr) -> syn::Result<Option<Expr>> {
    let value = text.value();
    if !value.contains(['{', '}']) {
        return Ok(None);
    }
    let mut output = String::new();
    let mut arguments = Vec::<Expr>::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if matches!(ch, '{' | '}') && chars.peek() == Some(&ch) {
            output.push(ch);
            output.push(ch);
            chars.next();
            continue;
        }
        if ch == '}' {
            return Err(syn::Error::new_spanned(
                text,
                "unmatched `}` in Text; use `}}` for a literal brace",
            ));
        }
        if ch != '{' {
            output.push(ch);
            continue;
        }
        let mut placeholder = String::new();
        let mut depth = 0;
        let mut closed = false;
        for next in chars.by_ref() {
            match next {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    closed = true;
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
            placeholder.push(next);
        }
        if !closed {
            return Err(syn::Error::new_spanned(
                text,
                "unclosed interpolation in Text",
            ));
        }
        let (source, format) = if syn::parse_str::<Expr>(&placeholder).is_ok() {
            (placeholder.as_str(), "")
        } else {
            let bytes = placeholder.as_bytes();
            let split = bytes
                .iter()
                .enumerate()
                .find(|(index, byte)| {
                    **byte == b':'
                        && (*index == 0 || bytes[*index - 1] != b':')
                        && bytes.get(*index + 1) != Some(&b':')
                })
                .map(|(index, _)| index);
            match split {
                Some(index) => (&placeholder[..index], &placeholder[index..]),
                None => (placeholder.as_str(), ""),
            }
        };
        let mut expr = syn::parse_str::<Expr>(source).map_err(|error| {
            syn::Error::new_spanned(text, format!("invalid Text interpolation: {error}"))
        })?;
        // Viewport exposes a computed category rather than a stored field.
        if let Expr::Field(field) = &expr
            && matches!(field.base.as_ref(),Expr::Path(path) if path.path.is_ident("viewport"))
            && matches!(&field.member,syn::Member::Named(name) if name=="breakpoint")
        {
            let base = &field.base;
            expr = syn::parse_quote!(#base.breakpoint());
        }
        output.push_str(&format!("{{{}{format}}}", arguments.len()));
        arguments.push(expr);
    }
    let format = LitStr::new(&output, text.span());
    Ok(Some(syn::parse_quote!(
        ::std::format!(#format, #(#arguments),*)
    )))
}

pub(crate) fn lower(tree: &mut ViewTree) -> syn::Result<()> {
    nodes(&mut tree.0)
}
fn nodes(items: &mut [ViewNode]) -> syn::Result<()> {
    for node in items {
        match node {
            ViewNode::Element(element) => {
                if element.name == "Text"
                    && let Some(argument) = element.arguments.first_mut()
                    && let Expr::Lit(literal) = &argument.value
                    && let Lit::Str(text) = &literal.lit
                    && let Some(expr) = expression(text)?
                {
                    argument.value = expr;
                }
                for modifier in &mut element.modifiers {
                    for argument in &mut modifier.arguments {
                        if let ModifierArg::Views(tree) = argument {
                            lower(tree)?;
                        }
                    }
                }
                match &mut element.content {
                    Content::Children(children) | Content::List { body: children, .. } => {
                        nodes(children)?
                    }
                    _ => {}
                }
            }
            ViewNode::Conditional(condition) => {
                nodes(&mut condition.then_branch)?;
                nodes(&mut condition.else_branch)?;
            }
            ViewNode::Loop(repeated) => nodes(&mut repeated.body)?,
        }
    }
    Ok(())
}
