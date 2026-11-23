//! Testable compiler pipeline behind Omen's procedural macros.
#![deny(missing_docs)]

mod ast;
mod codegen;
mod diagnostics;
pub mod editor;
pub mod formatter;
pub mod inspector;
mod interpolation;
mod models;
mod modifiers;
mod parser;
mod rewrite;
mod sugar;
mod validation;

use proc_macro2::TokenStream;
use quote::quote;

/// Parse, validate, and expand a view tree into public Omen builders.
///
/// # Errors
/// Returns spanned syntax or combined semantic diagnostics for invalid DSL input.
pub fn expand_view(tokens: TokenStream) -> syn::Result<TokenStream> {
    let tokens = parser::bindings(sugar::actions(tokens)?)?;
    if is_view_declaration(tokens.clone()) {
        let views = syn::parse2::<ast::NamedViews>(tokens)?;
        let mut output = TokenStream::new();
        for view in views.0 {
            output.extend(expand_named_view(view)?);
        }
        Ok(output)
    } else if tokens.clone().into_iter().next().is_some_and(|token| {
        matches!(token,proc_macro2::TokenTree::Punct(ref p) if p.as_char()=='@')
            || matches!(token,proc_macro2::TokenTree::Ident(ref i) if i=="let")
    }) {
        let mut view = syn::parse2::<ast::NamedView>(quote!(__OmenAnonymous() { #tokens }))?;
        interpolation::lower(&mut view.tree)?;
        validation::validate(&view.tree)?;
        sugar::named(&mut view);
        rewrite::named(&mut view)?;
        codegen::generate_anonymous(&view)
    } else {
        let mut tree = syn::parse2::<ast::ViewTree>(tokens)?;
        interpolation::lower(&mut tree)?;
        validation::validate(&tree)?;
        rewrite::anonymous(&mut tree);
        codegen::generate(&tree)
    }
}

fn expand_named_view(mut view: ast::NamedView) -> syn::Result<TokenStream> {
    if matches!(
        view.name.to_string().as_str(),
        "NavigationStack"
            | "NavigationBar"
            | "TabView"
            | "Tab"
            | "Text"
            | "Button"
            | "Layout"
            | "List"
            | "Image"
            | "Icon"
            | "TextField"
            | "SecureField"
            | "Checkbox"
            | "Toggle"
            | "Card"
            | "Progress"
            | "Divider"
            | "Gap"
    ) {
        return Err(syn::Error::new_spanned(
            &view.name,
            "this name is reserved for a built-in view",
        ));
    }
    let mut names = std::collections::HashSet::new();
    for property in &view.properties {
        if !names.insert(property.name.to_string()) {
            return Err(syn::Error::new_spanned(
                &property.name,
                "duplicate view property",
            ));
        }
    }
    interpolation::lower(&mut view.tree)?;
    validation::validate(&view.tree)?;
    sugar::named(&mut view);
    rewrite::named(&mut view)?;
    codegen::generate_named(&view)
}

/// Expand a reusable model declaration with typed, initialized fields.
///
/// # Errors
/// Returns a syntax error for invalid fields or uninferrable field types.
pub fn expand_state(tokens: TokenStream) -> syn::Result<TokenStream> {
    models::model(tokens)
}

/// Expand a reusable shared environment model declaration.
///
/// # Errors
/// Returns a syntax error for invalid model declarations.
pub fn expand_environment(tokens: TokenStream) -> syn::Result<TokenStream> {
    models::environment(tokens)
}

/// Expand a reusable translation catalog.
///
/// # Errors
/// Returns a syntax error for invalid or duplicate locales and translation keys.
pub fn expand_locale(tokens: TokenStream) -> syn::Result<TokenStream> {
    models::locale(tokens)
}

/// Determine whether tokens introduce a named view, for item-safe error emission.
pub fn is_view_declaration(tokens: TokenStream) -> bool {
    use syn::parse::Parser;
    let parser = |input: syn::parse::ParseStream<'_>| {
        let result = parser::declaration_ahead(input);
        let _: TokenStream = input.parse()?;
        Ok(result)
    };
    parser.parse2(tokens).unwrap_or(false)
}

/// Generate the native application entry point from one Rust view expression.
///
/// # Errors
/// Returns an error when the input is not a single expression.
pub fn expand_main(tokens: TokenStream) -> syn::Result<TokenStream> {
    let parsed = syn::parse2::<syn::Expr>(tokens.clone());
    let uppercase = tokens.clone().into_iter().next().is_some_and(|token| matches!(token,proc_macro2::TokenTree::Ident(name) if name.to_string().starts_with(char::is_uppercase)));
    let inline = matches!(tokens.clone().into_iter().next(),Some(proc_macro2::TokenTree::Punct(p)) if p.as_char()=='@');
    let expression = if uppercase || inline {
        expand_view(tokens)?
    } else {
        use quote::ToTokens;
        parsed?.into_token_stream()
    };
    Ok(quote! {
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        fn main() -> ::core::result::Result<(), ::omen_ui::__private::WindowError> {
            ::omen_ui::__private::run_view(#expression)
        }
        #[cfg(target_os="android")]
        #[unsafe(no_mangle)]
        pub fn android_main(app: ::omen_ui::__private::AndroidApp) {
            if let Err(error) = ::omen_ui::__private::run_android_view(app, #expression) {
                eprintln!("Omen Android startup failed: {error}");
            }
        }
        #[cfg(any(target_arch="wasm32",target_os="android"))]
        fn main() {}
        #[cfg(target_arch="wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn omen_app_start()->u32 {
            match ::omen_ui::__private::run_view(#expression) {Ok(())=>0,Err(_)=>1}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_logic_declarations_and_text_placeholders() {
        assert!(expand_state(quote!(Data { @State let value = Vec::new(); })).is_err());
        assert!(
            expand_state(quote!(Data { @State let value = 1; @State let value = 2; })).is_err()
        );
        assert!(expand_locale(quote!(Messages {en {title:"A",title:"B"}})).is_err());
        assert!(expand_view(quote!(Text("Unclosed {value"))).is_err());
    }

    #[test]
    fn ordinary_rust_push_constructors_remain_unchanged() {
        let expanded = expand_view(quote! {
            Home {
                @State rows = Vec::<i32>::new()
                Button("Add") { rows.push(Item()) }
            }
        })
        .unwrap();
        assert!(!expanded.to_string().contains("push_view"));
    }

    #[test]
    fn generated_control_flow_is_valid_rust_expression() {
        let expanded = expand_view(quote! {
            Layout {
                if true { Text("yes") } else { Layout {} }
                for (index, item) in ["a", "b"].iter().enumerate() { Text(format!("{index}: {item}")) }
                List(["a"]) { item in Text(item) }
            }.padding(16).background(.dark)
        }).unwrap();
        assert!(syn::parse2::<syn::Expr>(expanded).is_ok());
    }

    #[test]
    fn combines_independent_semantic_errors() {
        let errors =
            expand_view(quote! { Layout { Mystery(title: "a") Text("b").font(.nope).sparkle(1) } })
                .unwrap_err();
        assert_eq!(errors.into_iter().count(), 2);
    }

    #[test]
    fn validates_constructor_and_block_shapes() {
        for input in [
            quote!(Text()),
            quote!(Text("a", "b")),
            quote!(Text("a") {}),
            quote!(Button("a")),
            quote!(Layout(1) {}),
            quote!(List([1])),
            quote!(Text("a").spacing(1)),
            quote!(Text("a").padding(.blue)),
            quote!(Text("a").font()),
            quote!(Text("a").font(.title, .body)),
        ] {
            assert!(expand_view(input).is_err());
        }
    }

    #[test]
    fn expands_named_views_and_local_statements_into_rust_items() {
        let expanded = expand_view(quote! {
            pub Header(title: String) {
                let text = format!("Hello {title}");
                Layout { Text(text) }
            }
        })
        .unwrap();
        assert!(syn::parse2::<syn::File>(expanded).is_ok());
    }

    #[test]
    fn main_requires_one_expression() {
        assert!(expand_main(quote!(view! { HomeScreen() })).is_ok());
        assert!(expand_main(quote!()).is_err());
        assert!(expand_main(quote!(let x = 1;)).is_err());
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn marker_changes_studio_metadata_without_changing_rust_expansion() {
        let marked =
            "@Design HomeView { Layout { HeaderView() } } HeaderView { Text(\"Welcome\") }";
        let unmarked = marked.trim_start_matches("@Design ");
        assert_eq!(
            expand_view(marked.parse().unwrap()).unwrap().to_string(),
            expand_view(unmarked.parse().unwrap()).unwrap().to_string()
        );
        let legacy = marked.replacen("@Design", "@Preview", 1);
        assert!(expand_view(legacy.parse().unwrap()).is_err());
        assert_eq!(
            inspector::preview_marker_end(&format!("view! {{ {legacy} }}")).unwrap(),
            None
        );
        let source = format!("view! {{ {marked} }}");
        let end = inspector::preview_marker_end(&source).unwrap().unwrap();
        assert!(source[..end].ends_with("@Design"));
        let views = inspector::visual_declarations(&source).unwrap();
        assert_eq!(views.len(), 2);
        assert!(views[0].preview);
        assert!(!views[1].preview);
        let header = source.find("Text").unwrap();
        let node = inspector::inspect(&source, header).unwrap().unwrap();
        assert_eq!(node.path.first().map(String::as_str), Some("HeaderView"));
        let names: Vec<_> = editor::components(&source)
            .into_iter()
            .map(|component| component.name)
            .collect();
        assert_eq!(names, ["HomeView", "HeaderView"]);
    }

    #[test]
    fn grouped_views_allow_only_one_preview_marker() {
        let error = expand_view(
            "@Design First { Text(\"a\") } @Design Second { Text(\"b\") }"
                .parse()
                .unwrap(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("only one view may be marked"));
    }
}
