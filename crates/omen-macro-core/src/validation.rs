use crate::{ast::*, diagnostics::combine, modifiers};

pub(crate) fn validate(tree: &ViewTree) -> syn::Result<()> {
    let mut errors = None;
    validate_nodes(&tree.0, &mut errors);
    errors.map_or(Ok(()), Err)
}

fn validate_nodes(nodes: &[ViewNode], errors: &mut Option<syn::Error>) {
    for node in nodes {
        match node {
            ViewNode::Conditional(node) => {
                validate_nodes(&node.then_branch, errors);
                validate_nodes(&node.else_branch, errors);
            }
            ViewNode::Loop(node) => validate_nodes(&node.body, errors),
            ViewNode::Element(element) => {
                let name = element.name.to_string();

                let expected = match name.as_str() {
                    "Text" | "Button" | "List" | "Image" | "Icon" | "TextField" | "TextEditor"
                    | "SecureField" | "Checkbox" | "Toggle" | "Progress" => Some(1),
                    "Layout" | "AdaptiveLayout" | "AdaptiveGrid" | "Card" | "Divider" => Some(0),
                    "Gap" => Some(usize::from(!element.arguments.is_empty())),
                    "NavigationStack" => Some(if element.arguments.len() == 1 { 1 } else { 2 }),
                    "Tab" => Some(2),
                    "NavigationBar" | "TabView" => Some(1),
                    "VStack" | "HStack" | "ZStack" => {
                        combine(
                            errors,
                            syn::Error::new_spanned(
                                &element.name,
                                "use Layout with .mode(.horizontal), .mode(.vertical), or .mode(.stack)",
                            ),
                        );
                        None
                    }
                    _ => None,
                };
                let keyed_list = name == "List"
                    && element.arguments.len() == 2
                    && element.arguments[0].name.is_none()
                    && element.arguments[1]
                        .name
                        .as_ref()
                        .is_some_and(|name| name == "id");
                let mut names = std::collections::HashSet::new();
                let mut named = false;
                let mut positional = false;
                for argument in &element.arguments {
                    if let Some(name) = &argument.name {
                        named = true;
                        if !names.insert(name.to_string()) {
                            combine(
                                errors,
                                syn::Error::new_spanned(name, "duplicate view argument"),
                            );
                        }
                    } else {
                        positional = true;
                    }
                }
                if named && positional && !keyed_list {
                    combine(
                        errors,
                        syn::Error::new_spanned(
                            &element.name,
                            "cannot mix named and positional arguments",
                        ),
                    );
                }
                if expected.is_some() && named && !keyed_list {
                    combine(
                        errors,
                        syn::Error::new_spanned(
                            &element.name,
                            "built-in views use positional arguments",
                        ),
                    );
                }
                if expected.is_none() && positional {
                    combine(
                        errors,
                        syn::Error::new_spanned(
                            &element.name,
                            "reusable views use named arguments: ViewName(property: value)",
                        ),
                    );
                }
                if expected.is_none() && !matches!(element.content, Content::None) {
                    combine(
                        errors,
                        syn::Error::new_spanned(
                            &element.name,
                            "reusable views do not accept a child block",
                        ),
                    );
                }
                if let Some(expected) = expected {
                    if element.arguments.len() != expected && !keyed_list {
                        combine(
                            errors,
                            syn::Error::new_spanned(
                                &element.name,
                                format!("{name} requires {expected} constructor argument(s)"),
                            ),
                        );
                    }
                    let valid_content = matches!(
                        (name.as_str(), &element.content),
                        (
                            "Text"
                                | "Image"
                                | "Icon"
                                | "TextField"
                                | "TextEditor"
                                | "SecureField"
                                | "Checkbox"
                                | "Toggle"
                                | "Progress"
                                | "Divider"
                                | "Gap",
                            Content::None
                        ) | ("Button", Content::Action(_) | Content::AsyncAction(_))
                            | (
                                "Layout"
                                    | "AdaptiveLayout"
                                    | "AdaptiveGrid"
                                    | "Card"
                                    | "NavigationStack"
                                    | "NavigationBar"
                                    | "TabView"
                                    | "Tab",
                                Content::Children(_) | Content::None
                            )
                            | ("List", Content::List { .. })
                    );
                    if !valid_content {
                        let message = match name.as_str() {
                            "Text" => "Text cannot have a child block",
                            "Button" => {
                                "Button requires a Rust action block: Button(\"Title\") { action(); }"
                            }
                            "List" => {
                                "List requires a binding and rows: List(items) { item in Text(item) }"
                            }
                            _ => "this view does not accept this block type",
                        };
                        combine(errors, syn::Error::new_spanned(&element.name, message));
                    }
                }
                for modifier in &element.modifiers {
                    if let Err(error) = modifiers::generate(modifier, &name) {
                        combine(errors, error);
                    }
                }
                match &element.content {
                    Content::Children(nodes) | Content::List { body: nodes, .. } => {
                        validate_nodes(nodes, errors)
                    }
                    _ => {}
                }
            }
        }
    }
}
