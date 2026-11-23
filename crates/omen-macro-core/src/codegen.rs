use crate::{ast::*, modifiers};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

pub(crate) fn generate(tree: &ViewTree) -> syn::Result<TokenStream> {
    let expression = if let [node] = tree.0.as_slice() {
        generate_node(node, "root")?
    } else {
        let children = generate_children(&tree.0, "root")?;
        quote!(::omen_ui::AnyView::group(#children))
    };
    Ok(quote!({ use ::omen_ui::ViewExt as _; #expression }))
}
fn local(name: &str) -> syn::Ident {
    syn::Ident::new(name, Span::mixed_site())
}
fn with_site(expression: TokenStream, site: &str, span: Span) -> TokenStream {
    quote_spanned!(span=> (#expression).identity_site(concat!(module_path!(), ":", file!(), ":", line!(), ":", column!(), ":", #site)))
}
fn generate_children(nodes: &[ViewNode], site: &str) -> syn::Result<TokenStream> {
    let children = local("__omen_children");
    let pushes = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let expression = generate_node(node, &format!("{site}.{index}"))?;
            Ok(quote!(#children.push(::omen_ui::View::into_node(#expression));))
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote!({ let mut #children = ::std::vec::Vec::new(); #(#pushes)* #children }))
}
fn generate_node(node: &ViewNode, site: &str) -> syn::Result<TokenStream> {
    match node {
        ViewNode::Element(element) => generate_element(element, site),
        ViewNode::Conditional(node) => {
            let condition = &node.condition;
            let then_branch = generate_children(&node.then_branch, &format!("{site}.then"))?;
            let else_branch = generate_children(&node.else_branch, &format!("{site}.else"))?;
            let expression = quote_spanned!(node.span=> if #condition { ::omen_ui::AnyView::group(#then_branch) } else { ::omen_ui::AnyView::group(#else_branch) });
            Ok(with_site(expression, site, node.span))
        }
        ViewNode::Loop(node) => {
            let binding = &node.binding;
            let collection = &node.collection;
            let body = generate_children(&node.body, &format!("{site}.body"))?;
            let rows = local("__omen_rows");
            let expression = quote_spanned!(node.span=> { let mut #rows = ::std::vec::Vec::new(); for #binding in #collection { #rows.extend(#body); } ::omen_ui::AnyView::group(#rows) });
            Ok(with_site(expression, site, node.span))
        }
    }
}
fn generate_element(element: &ElementNode, site: &str) -> syn::Result<TokenStream> {
    let name = &element.name;
    let arguments = element
        .arguments
        .iter()
        .filter(|argument| {
            !argument
                .name
                .as_ref()
                .is_some_and(|name| name == "__omen_action")
        })
        .collect::<Vec<_>>();
    let builtin = crate::editor::BUILTIN_VIEW_NAMES.contains(&name.to_string().as_str());
    let constructor = if name == "NavigationStack" && arguments.len() == 1 {
        quote!(from_navigator)
    } else if name == "Gap" && arguments.len() == 1 {
        quote!(sized)
    } else {
        quote!(new)
    };
    let mut expression = if !builtin {
        let properties = arguments.iter().filter_map(|argument| argument.name.as_ref().map(|name| (name, &argument.value))).map(|(name, value)| quote_spanned!(name.span()=> #name: ::core::convert::Into::into(#value)));
        quote_spanned!(element.span=> #name { #(#properties),* })
    } else {
        match &element.content {
            Content::List { binding, body } => {
                let body = generate_children(body, &format!("{site}.rows"))?;
                let rows = local("__omen_list_rows");
                let items = local("__omen_list_items");
                let collection = &arguments[0].value;
                let fill = if let Some(key) = arguments.get(1) {
                    let key = &key.value;
                    let key_fn = local("__omen_key_fn");
                    let item = local("__omen_item");
                    let row_key = local("__omen_row_key");
                    quote! {
                        fn #key_fn<T, K>(item: &T, key: impl Fn(&T) -> K) -> K { key(item) }
                        for #item in #items {
                            let #row_key = #key_fn(&#item, #key); let #binding = #item;
                            #rows.push(::omen_ui::View::into_node(::omen_ui::AnyView::group(#body).key(#row_key)));
                        }
                    }
                } else {
                    quote! { for #binding in #items { #rows.push(::omen_ui::View::into_node(::omen_ui::AnyView::group(#body))); } }
                };
                quote_spanned!(element.span=> { let mut #rows = ::std::vec::Vec::new(); let #items = #collection; #fill ::omen_ui::List::new().children(#rows) })
            }
            Content::Action(_) | Content::AsyncAction(_) => {
                let factory = element
                    .arguments
                    .iter()
                    .find(|argument| {
                        argument
                            .name
                            .as_ref()
                            .is_some_and(|name| name == "__omen_action")
                    })
                    .map(|argument| &argument.value)
                    .ok_or_else(|| {
                        syn::Error::new(element.span, "action factory was not lowered")
                    })?;
                let constructor = if matches!(element.content, Content::AsyncAction(_)) {
                    quote!(new_async)
                } else {
                    quote!(new)
                };
                quote_spanned!(element.span=> ::omen_ui::#name::#constructor(#(#arguments),*, #factory))
            }
            Content::Children(nodes) => {
                let children = generate_children(nodes, &format!("{site}.children"))?;
                quote_spanned!(element.span=> ::omen_ui::#name::#constructor(#(#arguments),*).children(#children))
            }
            Content::None => {
                quote_spanned!(element.span=> ::omen_ui::#name::#constructor(#(#arguments),*))
            }
        }
    };
    expression = with_site(expression, site, element.span);
    for modifier in &element.modifiers {
        let modifier = modifiers::generate(modifier, &name.to_string())?;
        expression = quote!(#expression #modifier);
    }
    Ok(expression)
}
pub(crate) fn generate_named(view: &NamedView) -> syn::Result<TokenStream> {
    let NamedView {
        attributes,
        visibility,
        name,
        properties,
        statements,
        states,
        tree,
        ..
    } = view;
    let fields = properties.iter().map(|property| {
        let name = &property.name;
        let ty = &property.ty;
        quote!(#[doc(hidden)] pub #name: #ty)
    });
    let props = local("__omen_properties");
    let scope = local("__omen_scope");
    let bindings = properties.iter().map(|property| {
        let name = &property.name;
        quote!(let #name = #props.#name.clone();)
    });
    let body = state_body(statements, states, &scope);
    let expression = generate(tree)?;
    let fallback_documentation = if attributes.is_empty() {
        quote!(#[doc="A dependency-tracked Omen view declared with view!."])
    } else {
        quote!()
    };
    Ok(quote! {
        #fallback_documentation
        #(#attributes)*
        #visibility struct #name { #(#fields),* }
        impl ::omen_ui::View for #name {
            fn into_node(self) -> ::omen_ui::ViewNode {
                let #props = ::std::rc::Rc::new(self);
                ::omen_ui::ViewNode::new(::omen_ui::ViewKind::Reactive(::omen_ui::ReactiveView::new(
                    ::std::any::type_name::<Self>(), move |#scope| {
                        use ::omen_ui::BoolExt as _;
                        #(#bindings)* #(#body)*
                        Ok(::omen_ui::View::into_node(#expression))
                    }
                )))
            }
        }
    })
}

fn state_body(
    statements: &[syn::Stmt],
    states: &[StateDecl],
    scope: &syn::Ident,
) -> Vec<TokenStream> {
    let mut body = Vec::new();
    for index in 0..=statements.len() {
        for state in states.iter().filter(|state| state.statement_index == index) {
            let name = &state.name;
            let slot = name.to_string();
            if state.responsive {
                body.push(quote!(let #name = #scope.responsive();));
            } else if state.environment {
                let ty = &state.ty;
                body.push(quote!(let #name = #scope.environment::<#ty>()?;));
            } else {
                let initialize = &state.initialize;
                if let Some(ty) = &state.ty {
                    body.push(quote!(let #name = #scope.state::<#ty>(#slot, || #initialize)?;));
                } else {
                    body.push(quote!(let #name = #scope.state(#slot, || #initialize)?;));
                }
            }
        }
        if let Some(statement) = statements.get(index) {
            body.push(quote!(#statement));
        }
    }
    body
}

pub(crate) fn generate_anonymous(view: &NamedView) -> syn::Result<TokenStream> {
    let scope = local("__omen_scope");
    let body = state_body(&view.statements, &view.states, &scope);
    let expression = generate(&view.tree)?;
    Ok(quote!({
        ::omen_ui::AnyView::new(::omen_ui::ViewNode::new(::omen_ui::ViewKind::Reactive(::omen_ui::ReactiveView::new(
            concat!(module_path!(), ":", file!(), ":", line!(), ":", column!(), ":anonymous"), move |#scope| {
                use ::omen_ui::BoolExt as _;
                #(#body)*
                Ok(::omen_ui::View::into_node(#expression))
            }
        ))))
    }))
}
