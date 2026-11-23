//! Lower compact DSL expressions before reactive state rewriting.
use crate::ast::*;
use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Expr, Token, parse::Parser, punctuated::Punctuated, visit_mut::VisitMut};

/// Named view arguments inside a navigation action are ordinary generated fields.
pub(crate) fn actions(tokens: TokenStream) -> syn::Result<TokenStream> {
    let input = tokens.into_iter().collect::<Vec<_>>();
    let mut output = TokenStream::new();
    let mut index = 0;
    while index < input.len() {
        if let TokenTree::Ident(method) = &input[index]
            && method == "push"
            && index > 0
            && matches!(&input[index-1], TokenTree::Punct(p) if p.as_char()=='.')
            && let Some(TokenTree::Group(arguments)) = input.get(index + 1)
            && arguments.delimiter() == Delimiter::Parenthesis
        {
            let route = arguments.stream().into_iter().collect::<Vec<_>>();
            if let [TokenTree::Ident(name), TokenTree::Group(properties)] = route.as_slice()
                && name.to_string().starts_with(char::is_uppercase)
                && properties.delimiter() == Delimiter::Parenthesis
            {
                let fields = Punctuated::<Argument, Token![,]>::parse_terminated
                    .parse2(properties.stream())?;
                if !fields.is_empty() && fields.iter().all(|field| field.name.is_some()) {
                    let fields = fields.iter().map(|field| {
                        let key = &field.name;
                        let value = &field.value;
                        quote!(#key: ::core::convert::Into::into(#value))
                    });
                    output.extend(quote!(push_view(#name { #(#fields),* })));
                    index += 2;
                    continue;
                }
            }
        }
        output.extend([match &input[index] {
            TokenTree::Group(group) => {
                let mut nested = Group::new(group.delimiter(), actions(group.stream())?);
                nested.set_span(group.span());
                TokenTree::Group(nested)
            }
            token => token.clone(),
        }]);
        index += 1;
    }
    Ok(output)
}

struct Localized(Vec<String>, Vec<String>);
impl VisitMut for Localized {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Macro(mac) = expression
            && mac
                .mac
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "format" || segment.ident == "format_args")
            && let Ok(mut arguments) =
                Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.mac.tokens.clone())
        {
            for argument in &mut arguments {
                if let Expr::Assign(named) = argument {
                    self.visit_expr_mut(&mut named.right);
                } else {
                    self.visit_expr_mut(argument);
                }
            }
            mac.mac.tokens = arguments.into_token_stream();
            return;
        }
        if let Expr::Assign(assign) = expression
            && let Expr::Field(field) = assign.left.as_ref()
            && let Expr::Path(path) = field.base.as_ref()
            && self.0.iter().any(|name| path.path.is_ident(name))
            && matches!(&field.member,syn::Member::Named(name) if name=="locale")
        {
            let base = &field.base;
            let value = &assign.right;
            *expression = syn::parse_quote!(#base.set_locale(::omen_ui::Locale::new(#value)));
            return;
        }
        if let Expr::Field(field) = expression
            && let Expr::Path(path) = field.base.as_ref()
            && self.0.iter().any(|name| path.path.is_ident(name))
            && let syn::Member::Named(name) = &field.member
        {
            let base = &field.base;
            *expression = if name == "locale" {
                syn::parse_quote!(#base.locale())
            } else {
                if self.1.iter().any(|name| path.path.is_ident(name)) {
                    syn::parse_quote!(#base.#name())
                } else {
                    let key = name.to_string();
                    syn::parse_quote!(#base.text(#key))
                }
            };
            return;
        }
        syn::visit_mut::visit_expr_mut(self, expression);
    }
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        let saved = self.0.clone();
        for statement in &mut block.stmts {
            self.visit_stmt_mut(statement);
            if let syn::Stmt::Local(local) = statement
                && let syn::Pat::Ident(binding) = &local.pat
            {
                self.0.retain(|name| name != &binding.ident.to_string());
            }
        }
        self.0 = saved;
    }
}
fn nodes(items: &mut [ViewNode], rewrite: &mut Localized) {
    for node in items {
        match node {
            ViewNode::Element(element) => {
                for argument in &mut element.arguments {
                    rewrite.visit_expr_mut(&mut argument.value);
                }
                for modifier in &mut element.modifiers {
                    for argument in &mut modifier.arguments {
                        match argument {
                            ModifierArg::Views(tree) => nodes(&mut tree.0, rewrite),
                            ModifierArg::Rust(expr) => rewrite.visit_expr_mut(expr),
                            ModifierArg::Action { block, .. } => rewrite.visit_block_mut(block),
                            ModifierArg::Symbol(_) | ModifierArg::Adaptive(_) => {}
                        }
                    }
                }
                match &mut element.content {
                    Content::Children(children) | Content::List { body: children, .. } => {
                        nodes(children, rewrite)
                    }
                    Content::Action(block) | Content::AsyncAction(block) => {
                        rewrite.visit_block_mut(block)
                    }
                    Content::None => {}
                }
            }
            ViewNode::Conditional(condition) => {
                rewrite.visit_expr_mut(&mut condition.condition);
                nodes(&mut condition.then_branch, rewrite);
                nodes(&mut condition.else_branch, rewrite);
            }
            ViewNode::Loop(repeated) => {
                rewrite.visit_expr_mut(&mut repeated.collection);
                nodes(&mut repeated.body, rewrite);
            }
        }
    }
}
pub(crate) fn named(view: &mut NamedView) {
    let mut rewrite = Localized(
        view.states
            .iter()
            .filter(|state| state.localized)
            .map(|state| state.name.to_string())
            .collect(),
        view.states
            .iter()
            .filter(|state| state.localized && state.ty.is_none())
            .map(|state| state.name.to_string())
            .collect(),
    );
    for statement in &mut view.statements {
        rewrite.visit_stmt_mut(statement);
    }
    nodes(&mut view.tree.0, &mut rewrite);
}
