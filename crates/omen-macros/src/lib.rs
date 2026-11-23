//! Omen's UI and reusable logic declaration macros.
#![deny(missing_docs)]
use proc_macro::TokenStream;

/// Define a named view or construct an anonymous view expression.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let tokens = proc_macro2::TokenStream::from(input);
    let declaration = omen_macro_core::is_view_declaration(tokens.clone());
    match omen_macro_core::expand_view(tokens) {
        Ok(expanded) => public_paths(expanded).into(),
        Err(error) => {
            let errors = error.into_compile_error();
            if declaration {
                errors.into()
            } else {
                quote::quote!({ #errors }).into()
            }
        }
    }
}

/// Start a native application displaying a view expression.
#[proc_macro]
pub fn main(input: TokenStream) -> TokenStream {
    match omen_macro_core::expand_main(input.into()) {
        Ok(expanded) => public_paths(expanded).into(),
        Err(error) => {
            let errors = error.into_compile_error();
            quote::quote!(fn main() { #errors }).into()
        }
    }
}

/// Declare a reusable reactive state model, alongside views or in any Rust module.
#[proc_macro]
pub fn state(input: TokenStream) -> TokenStream {
    match omen_macro_core::expand_state(input.into()) {
        Ok(tokens) => public_paths(tokens).into(),
        Err(error) => error.into_compile_error().into(),
    }
}
/// Declare a shared model for injection with a view's environment modifier.
#[proc_macro]
pub fn environment(input: TokenStream) -> TokenStream {
    match omen_macro_core::expand_environment(input.into()) {
        Ok(tokens) => public_paths(tokens).into(),
        Err(error) => error.into_compile_error().into(),
    }
}
/// Declare a reusable collection of translations.
#[proc_macro]
pub fn locale(input: TokenStream) -> TokenStream {
    match omen_macro_core::expand_locale(input.into()) {
        Ok(tokens) => public_paths(tokens).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

// Resolve the consumer's dependency, including a renamed dependency. The AST
// compiler retains its compatibility path; applications can depend on omen alone.
fn public_paths(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::{Group, Ident, TokenTree};
    let name = ["omen", "omen-ui"]
        .into_iter()
        .find_map(|package| {
            proc_macro_crate::crate_name(package)
                .ok()
                .map(|found| match found {
                    proc_macro_crate::FoundCrate::Itself => package.replace('-', "_"),
                    proc_macro_crate::FoundCrate::Name(name) => name,
                })
        })
        .unwrap_or_else(|| "omen_ui".into());
    fn rewrite(tokens: proc_macro2::TokenStream, name: &str) -> proc_macro2::TokenStream {
        let mut output = proc_macro2::TokenStream::new();
        let mut colons = 0;
        for token in tokens {
            match token {
                TokenTree::Group(group) => {
                    let mut new = Group::new(group.delimiter(), rewrite(group.stream(), name));
                    new.set_span(group.span());
                    output.extend([TokenTree::Group(new)]);
                    colons = 0;
                }
                TokenTree::Ident(ident) if colons >= 2 && ident == "omen_ui" => {
                    output.extend([TokenTree::Ident(Ident::new(name, ident.span()))]);
                    colons = 0;
                }
                token => {
                    colons = if matches!(&token,TokenTree::Punct(p) if p.as_char()==':') {
                        colons + 1
                    } else {
                        0
                    };
                    output.extend([token]);
                }
            }
        }
        output
    }
    rewrite(tokens, &name)
}
