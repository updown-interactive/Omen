use crate::ast::*;
use syn::{
    Token, braced, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token,
};

impl Parse for ViewTree {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let nodes = parse_nodes(input)?;
        if nodes.is_empty() {
            return Err(input.error("view! requires at least one view"));
        }
        Ok(Self(nodes))
    }
}

fn parse_nodes(input: ParseStream<'_>) -> syn::Result<Vec<ViewNode>> {
    let mut nodes = Vec::new();
    while !input.is_empty() {
        nodes.push(if input.peek(Token![if]) {
            ViewNode::Conditional(Box::new(parse_conditional(input)?))
        } else if input.peek(Token![for]) {
            let keyword: Token![for] = input.parse()?;
            let binding = syn::Pat::parse_multi_with_leading_vert(input)?;
            input.parse::<Token![in]>()?;
            let collection = syn::Expr::parse_without_eager_brace(input)?;
            let body;
            braced!(body in input);
            ViewNode::Loop(Box::new(LoopNode {
                binding,
                collection,
                body: parse_nodes(&body)?,
                span: keyword.span,
            }))
        } else {
            ViewNode::Element(Box::new(parse_element(input)?))
        });
    }
    Ok(nodes)
}

fn parse_conditional(input: ParseStream<'_>) -> syn::Result<ConditionalNode> {
    let keyword: Token![if] = input.parse()?;
    let condition = syn::Expr::parse_without_eager_brace(input)?;
    let body;
    braced!(body in input);
    let then_branch = parse_nodes(&body)?;
    let else_branch = if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
            vec![ViewNode::Conditional(Box::new(parse_conditional(input)?))]
        } else {
            let body;
            braced!(body in input);
            parse_nodes(&body)?
        }
    } else {
        Vec::new()
    };
    Ok(ConditionalNode {
        condition,
        then_branch,
        else_branch,
        span: keyword.span,
    })
}

fn parse_element(input: ParseStream<'_>) -> syn::Result<ElementNode> {
    let name: syn::Ident = input.parse()?;
    let span = name.span();
    let (mut arguments, paren_span) = if input.peek(token::Paren) {
        let args;
        let paren = parenthesized!(args in input);
        let list: Vec<Argument> = Punctuated::<Argument, Token![,]>::parse_terminated(&args)?
            .into_iter()
            .collect();
        (list, Some(paren.span))
    } else {
        (Vec::new(), None)
    };
    let content = if name == "Button" && input.peek(Token![async]) {
        input.parse::<Token![async]>()?;
        if input.peek(Token![move]) {
            input.parse::<Token![move]>()?;
        }
        Content::AsyncAction(input.parse()?)
    } else if input.peek(token::Brace) {
        if name == "Button" {
            Content::Action(input.parse()?)
        } else {
            let body;
            braced!(body in input);
            if name == "List" {
                let binding = syn::Pat::parse_multi_with_leading_vert(&body)?;
                body.parse::<Token![in]>()?;
                Content::List {
                    binding: Box::new(binding),
                    body: parse_nodes(&body)?,
                }
            } else {
                Content::Children(parse_nodes(&body)?)
            }
        }
    } else {
        Content::None
    };
    let mut modifiers = Vec::new();
    if matches!(name.to_string().as_str(), "Toggle" | "Checkbox")
        && arguments.len() == 2
        && arguments.iter().all(|argument| argument.name.is_none())
        && let Some(binding) = arguments.pop()
    {
        modifiers.push(ModifierNode {
            name: syn::Ident::new("checked", span),
            arguments: vec![ModifierArg::Rust(binding.value)],
            span,
        });
    }
    while input.peek(Token![.]) {
        let dot: Token![.] = input.parse()?;
        let mut name: syn::Ident = input.parse()?;
        let asynchronous = if input.peek(Token![async]) {
            input.parse::<Token![async]>()?;
            true
        } else {
            false
        };
        if asynchronous && name == "onTap" {
            name = syn::Ident::new("onTapAsync", name.span());
        }
        if asynchronous && name == "onAppear" {
            name = syn::Ident::new("task", name.span());
        }
        let mut arguments = if asynchronous || input.peek(token::Brace) {
            vec![ModifierArg::Action {
                block: input.parse()?,
                asynchronous,
            }]
        } else {
            let args;
            parenthesized!(args in input);
            if name == "offset" && args.peek(syn::Ident) && args.peek2(Token![:]) {
                let values = Punctuated::<Argument, Token![,]>::parse_terminated(&args)?;
                let mut x = None;
                let mut y = None;
                for argument in values {
                    match argument.name.as_ref().map(ToString::to_string).as_deref() {
                        Some("x") if x.is_none() => x = Some(argument.value),
                        Some("y") if y.is_none() => y = Some(argument.value),
                        _ => return Err(args.error("offset accepts unique x and y arguments")),
                    }
                }
                vec![
                    ModifierArg::Rust(x.unwrap_or_else(|| syn::parse_quote!(0.0))),
                    ModifierArg::Rust(y.unwrap_or_else(|| syn::parse_quote!(0.0))),
                ]
            } else if args.peek(Token![.]) {
                Punctuated::<ModifierArg, Token![,]>::parse_terminated(&args)?
                    .into_iter()
                    .collect()
            } else {
                let tokens: proc_macro2::TokenStream = args.parse()?;
                let tokens = if matches!(tokens.clone().into_iter().next(), Some(proc_macro2::TokenTree::Punct(p)) if p.as_char()=='.')
                {
                    tokens
                } else {
                    crate::modifiers::expression_symbols(&name, tokens)?
                };
                use syn::parse::Parser;
                Punctuated::<ModifierArg, Token![,]>::parse_terminated
                    .parse2(tokens)?
                    .into_iter()
                    .collect()
            }
        };
        if matches!(
            name.to_string().as_str(),
            "sheet" | "dialog" | "popover" | "modal"
        ) && input.peek(token::Brace)
        {
            let content;
            braced!(content in input);
            arguments.push(ModifierArg::Views(ViewTree(parse_nodes(&content)?)));
        }
        modifiers.push(ModifierNode {
            name,
            arguments,
            span: dot.span,
        });
    }
    Ok(ElementNode {
        name,
        arguments,
        paren_span,
        content,
        modifiers,
        span,
    })
}

impl Parse for ModifierArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(Token![.]) && !input.peek(Token![..]) {
            input.parse::<Token![.]>()?;
            let symbol: syn::Ident = input.parse()?;
            if input.peek(token::Paren) {
                let duration;
                parenthesized!(duration in input);
                if symbol == "adaptive" {
                    use omen_core::LayoutMode;
                    let mut compact = None;
                    let mut medium = None;
                    let mut expanded = None;
                    while !duration.is_empty() {
                        let key: syn::Ident = duration.parse()?;
                        duration.parse::<Token![:]>()?;
                        duration.parse::<Token![.]>()?;
                        let mode: syn::Ident = duration.parse()?;
                        let value = match mode.to_string().as_str() {
                            "vertical" => LayoutMode::Vertical,
                            "horizontal" => LayoutMode::Horizontal,
                            "stack" => LayoutMode::Stack,
                            _ => {
                                return Err(syn::Error::new_spanned(
                                    mode,
                                    "expected .vertical, .horizontal or .stack",
                                ));
                            }
                        };
                        let slot = match key.to_string().as_str() {
                            "compact" => &mut compact,
                            "medium" => &mut medium,
                            "expanded" => &mut expanded,
                            _ => {
                                return Err(syn::Error::new_spanned(
                                    key,
                                    "expected compact, medium or expanded",
                                ));
                            }
                        };
                        if slot.replace(value).is_some() {
                            return Err(syn::Error::new_spanned(
                                key,
                                "duplicate adaptive width category",
                            ));
                        }
                        if !duration.is_empty() {
                            duration.parse::<Token![,]>()?;
                        }
                    }
                    let compact =
                        compact.ok_or_else(|| duration.error("missing compact direction"))?;
                    let expanded =
                        expanded.ok_or_else(|| duration.error("missing expanded direction"))?;
                    return Ok(Self::Adaptive(omen_core::AdaptiveDirection {
                        compact,
                        medium,
                        expanded,
                    }));
                }
                if matches!(
                    symbol.to_string().as_str(),
                    "currency"
                        | "number"
                        | "percent"
                        | "date"
                        | "time"
                        | "datetime"
                        | "relative"
                        | "duration"
                        | "measurement"
                        | "bytes"
                        | "data_rate"
                        | "text"
                        | "list"
                        | "phone"
                        | "address"
                ) {
                    let tokens: proc_macro2::TokenStream = duration.parse()?;
                    let method = syn::Ident::new(&symbol.to_string(), symbol.span());
                    let token_str = tokens.to_string().replace(' ', "");
                    let mut expr: syn::Expr = match symbol.to_string().as_str() {
                        "date" => match token_str.as_str() {
                            ".short" => syn::parse_quote!(::omen_ui::format::FormatOption::date(::omen_ui::format::DateStyle::Short)),
                            ".medium" => syn::parse_quote!(::omen_ui::format::FormatOption::date(::omen_ui::format::DateStyle::Medium)),
                            ".long" => syn::parse_quote!(::omen_ui::format::FormatOption::date(::omen_ui::format::DateStyle::Long)),
                            ".full" => syn::parse_quote!(::omen_ui::format::FormatOption::date(::omen_ui::format::DateStyle::Full)),
                            _ if tokens.is_empty() => syn::parse_quote!(::omen_ui::format::FormatOption::date(::omen_ui::format::DateStyle::Medium)),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::date(#tokens)),
                        },
                        "datetime" => match token_str.as_str() {
                            ".short" => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(::omen_ui::format::DateStyle::Short)),
                            ".medium" => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(::omen_ui::format::DateStyle::Medium)),
                            ".long" => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(::omen_ui::format::DateStyle::Long)),
                            ".full" => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(::omen_ui::format::DateStyle::Full)),
                            _ if tokens.is_empty() => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(::omen_ui::format::DateStyle::Medium)),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::datetime(#tokens)),
                        },
                        "number" => match token_str.as_str() {
                            ".compact" => syn::parse_quote!(::omen_ui::format::FormatOption::number_compact()),
                            _ if tokens.is_empty() => syn::parse_quote!(::omen_ui::format::FormatOption::number()),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::number(#tokens)),
                        },
                        "percent" => match token_str.as_str() {
                            ".whole" => syn::parse_quote!(::omen_ui::format::FormatOption::percent_whole()),
                            _ if tokens.is_empty() => syn::parse_quote!(::omen_ui::format::FormatOption::percent()),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::percent(#tokens)),
                        },
                        "relative" => match token_str.as_str() {
                            ".numeric" => syn::parse_quote!(::omen_ui::format::FormatOption::relative_numeric()),
                            _ if tokens.is_empty() => syn::parse_quote!(::omen_ui::format::FormatOption::relative()),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::relative(#tokens)),
                        },
                        "duration" => match token_str.as_str() {
                            ".clock" => syn::parse_quote!(::omen_ui::format::FormatOption::duration(::omen_ui::format::DurationStyle::Clock)),
                            ".human" => syn::parse_quote!(::omen_ui::format::FormatOption::duration(::omen_ui::format::DurationStyle::Human)),
                            ".short" => syn::parse_quote!(::omen_ui::format::FormatOption::duration(::omen_ui::format::DurationStyle::Short)),
                            ".narrow" => syn::parse_quote!(::omen_ui::format::FormatOption::duration(::omen_ui::format::DurationStyle::Narrow)),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::duration(#tokens)),
                        },
                        "bytes" => match token_str.as_str() {
                            ".binary" => syn::parse_quote!(::omen_ui::format::FormatOption::bytes(::omen_ui::format::ByteStandard::Binary)),
                            ".decimal" => syn::parse_quote!(::omen_ui::format::FormatOption::bytes(::omen_ui::format::ByteStandard::Decimal)),
                            _ => syn::parse_quote!(::omen_ui::format::FormatOption::bytes(#tokens)),
                        },
                        "measurement" => {
                            let unit_expr = match token_str.as_str() {
                                ".kilometer" | ".kilometers" => quote::quote!(::omen_ui::format::Unit::Kilometer),
                                ".meter" | ".meters" => quote::quote!(::omen_ui::format::Unit::Meter),
                                ".centimeter" | ".centimeters" => quote::quote!(::omen_ui::format::Unit::Centimeter),
                                ".millimeter" | ".millimeters" => quote::quote!(::omen_ui::format::Unit::Millimeter),
                                ".mile" | ".miles" => quote::quote!(::omen_ui::format::Unit::Mile),
                                ".yard" | ".yards" => quote::quote!(::omen_ui::format::Unit::Yard),
                                ".foot" | ".feet" => quote::quote!(::omen_ui::format::Unit::Foot),
                                ".inch" | ".inches" => quote::quote!(::omen_ui::format::Unit::Inch),
                                ".kilogram" | ".kilograms" => quote::quote!(::omen_ui::format::Unit::Kilogram),
                                ".gram" | ".grams" => quote::quote!(::omen_ui::format::Unit::Gram),
                                ".pound" | ".pounds" => quote::quote!(::omen_ui::format::Unit::Pound),
                                ".ounce" | ".ounces" => quote::quote!(::omen_ui::format::Unit::Ounce),
                                ".celsius" => quote::quote!(::omen_ui::format::Unit::Celsius),
                                ".fahrenheit" => quote::quote!(::omen_ui::format::Unit::Fahrenheit),
                                ".kelvin" => quote::quote!(::omen_ui::format::Unit::Kelvin),
                                _ => tokens.clone(),
                            };
                            syn::parse_quote!(::omen_ui::format::FormatOption::measurement(#unit_expr))
                        }
                        _ => {
                            if tokens.is_empty() {
                                syn::parse_quote!(::omen_ui::format::FormatOption::#method())
                            } else {
                                syn::parse_quote!(::omen_ui::format::FormatOption::#method(#tokens))
                            }
                        }
                    };

                    while input.peek(Token![.]) && !input.peek(Token![..]) {
                        input.parse::<Token![.]>()?;
                        let chain_ident: syn::Ident = input.parse()?;
                        if input.peek(token::Paren) {
                            let chain_group;
                            syn::parenthesized!(chain_group in input);
                            let chain_tokens: proc_macro2::TokenStream = chain_group.parse()?;
                            expr = syn::parse_quote!(#expr.#chain_ident(#chain_tokens));
                        } else {
                            expr = syn::parse_quote!(#expr.#chain_ident());
                        }
                    }
                    return Ok(Self::Rust(expr));
                }
                let tokens: proc_macro2::TokenStream = duration.parse()?;
                let value = tokens.to_string().replace(' ', "");
                let millis = if let Some(value) = value.strip_suffix(".ms") {
                    value.parse::<u64>().ok()
                } else if let Some(value) = value.strip_suffix(".s") {
                    value.parse::<u64>().ok().and_then(|n| n.checked_mul(1000))
                } else {
                    None
                }
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        tokens,
                        "expected an integer duration such as 800.ms or 1.s",
                    )
                })?;
                let duration = quote::quote!(::std::time::Duration::from_millis(#millis));
                let expression = match symbol.to_string().as_str() {
                    "spring" => {
                        syn::parse_quote!(::omen_ui::Animation::spring(#duration,::omen_ui::Spring::default()))
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            symbol,
                            "unknown animation; use .spring(800.ms)",
                        ));
                    }
                };
                Ok(Self::Rust(expression))
            } else {
                Ok(Self::Symbol(symbol))
            }
        } else {
            Ok(Self::Rust(input.parse()?))
        }
    }
}

impl Parse for Argument {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = if input.peek(syn::Ident) && input.peek2(Token![:]) && !input.peek2(Token![::]) {
            let name = input.parse()?;
            input.parse::<Token![:]>()?;
            Some(name)
        } else {
            None
        };
        Ok(Self {
            name,
            value: input.parse()?,
        })
    }
}
impl Parse for Property {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        Ok(Self {
            name,
            ty: input.parse()?,
        })
    }
}

pub(crate) fn declaration_ahead(input: ParseStream<'_>) -> bool {
    let ahead = input.fork();
    if ahead.call(syn::Attribute::parse_outer).is_err() {
        return false;
    }
    if ahead.peek(Token![pub]) || preview_ahead(&ahead) {
        return true;
    }
    let Ok(name) = ahead.parse::<syn::Ident>() else {
        return false;
    };
    if crate::editor::BUILTIN_VIEW_NAMES.contains(&name.to_string().as_str())
        || matches!(name.to_string().as_str(), "VStack" | "HStack" | "ZStack")
    {
        return false;
    }
    if ahead.peek(token::Brace) {
        return true;
    }
    if !ahead.peek(token::Paren) {
        return false;
    }
    let Ok(group) = ahead.parse::<proc_macro2::Group>() else {
        return false;
    };
    group.delimiter() == proc_macro2::Delimiter::Parenthesis && ahead.peek(token::Brace)
}

pub(crate) fn is_design_marker(marker: &syn::Ident) -> bool {
    marker == "Design"
}

fn preview_ahead(input: ParseStream<'_>) -> bool {
    let ahead = input.fork();
    ahead.parse::<Token![@]>().is_ok()
        && ahead
            .parse::<syn::Ident>()
            .is_ok_and(|marker| is_design_marker(&marker))
}

impl Parse for NamedViews {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut views = Vec::new();
        let mut preview_found = false;
        let mut names = std::collections::HashSet::new();
        while !input.is_empty() {
            let view: NamedView = input.parse()?;
            if let Some(span) = view.preview {
                if preview_found {
                    return Err(syn::Error::new(span, "only one view may be marked @Design"));
                }
                preview_found = true;
            }
            if !names.insert(view.name.to_string()) {
                return Err(syn::Error::new_spanned(
                    &view.name,
                    "duplicate view declaration",
                ));
            }
            views.push(view);
        }
        Ok(Self(views))
    }
}

impl Parse for NamedView {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let start = input.span().byte_range().start;
        let mut attributes = input.call(syn::Attribute::parse_outer)?;
        let preview = if preview_ahead(input) {
            input.parse::<Token![@]>()?;
            let marker: syn::Ident = input.parse()?;
            attributes.extend(input.call(syn::Attribute::parse_outer)?);
            Some(marker.span())
        } else {
            None
        };
        if let Some(attribute) = attributes
            .iter()
            .find(|attribute| !attribute.path().is_ident("doc"))
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "named views currently support documentation attributes only",
            ));
        }
        let visibility = input.parse()?;
        let name = input.parse()?;
        let properties = if input.peek(token::Paren) {
            let properties;
            parenthesized!(properties in input);
            Punctuated::<Property, Token![,]>::parse_terminated(&properties)?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };
        let body;
        let brace = braced!(body in input);
        let mut statements = Vec::new();
        let mut states = Vec::new();
        while !body.is_empty() {
            if body.peek(Token![@]) {
                body.parse::<Token![@]>()?;
                let marker: syn::Ident = body.parse()?;
                if marker == "Localize" {
                    let name: syn::Ident = body.parse()?;
                    let mut typed = false;
                    let initialize = if body.peek(Token![=]) {
                        body.parse::<Token![=]>()?;
                        let mut expression: syn::Expr = body.parse()?;
                        if let syn::Expr::Call(call) = &expression
                            && let syn::Expr::Path(path) = call.func.as_ref()
                            && call.args.is_empty()
                            && path.path.segments.last().is_some_and(|segment| {
                                segment.ident.to_string().starts_with(char::is_uppercase)
                            })
                        {
                            typed = true;
                            expression = syn::parse_quote!(#path::typed());
                        }
                        if body.peek(Token![;]) {
                            body.parse::<Token![;]>()?;
                        }
                        if typed {
                            expression
                        } else {
                            syn::parse_quote!(::core::convert::Into::<::omen_ui::Localizer>::into(#expression))
                        }
                    } else {
                        let catalogs;
                        braced!(catalogs in body);
                        crate::models::catalog(&catalogs)?
                    };
                    states.push(StateDecl {
                        name,
                        ty: if typed {
                            None
                        } else {
                            Some(syn::parse_quote!(::omen_ui::Localizer))
                        },
                        initialize: Some(initialize),
                        environment: false,
                        responsive: false,
                        localized: true,
                        statement_index: statements.len(),
                    });
                    continue;
                }
                let responsive = marker == "Responsive";
                let environment = if marker == "State" || responsive {
                    false
                } else if marker == "Environment" {
                    true
                } else {
                    return Err(syn::Error::new_spanned(
                        marker,
                        "expected @State, @Environment or @Responsive",
                    ));
                };
                if body.peek(Token![let]) {
                    body.parse::<Token![let]>()?;
                }
                let name: syn::Ident = body.parse()?;
                let mut ty = if body.peek(Token![:]) {
                    body.parse::<Token![:]>()?;
                    Some(body.parse()?)
                } else {
                    None
                };
                let initialize = if body.peek(Token![=]) {
                    body.parse::<Token![=]>()?;
                    Some(body.parse()?)
                } else {
                    None
                };
                let initialize = initialize.map(|expr: syn::Expr| {
                    if let syn::Expr::Call(call) = &expr
                        && call.args.is_empty()
                        && let syn::Expr::Path(path) = call.func.as_ref()
                        && path.qself.is_none()
                        && path.path.segments.last().is_some_and(|segment| {
                            segment.ident.to_string().starts_with(char::is_uppercase)
                        })
                    {
                        if path.path.is_ident("Navigator") {
                            syn::parse_quote!(::omen_ui::Navigator::new())
                        } else {
                            syn::parse_quote!(#path::new())
                        }
                    } else {
                        expr
                    }
                });
                if body.peek(Token![;]) {
                    body.parse::<Token![;]>()?;
                }
                if environment && ty.is_none() {
                    ty = match name.to_string().as_str() {
                        "env" | "device" => Some(syn::parse_quote!(::omen_ui::DeviceEnvironment)),
                        "viewport" => Some(syn::parse_quote!(::omen_ui::Viewport)),
                        "lifecycle" => Some(syn::parse_quote!(::omen_ui::AppLifecycle)),
                        "app" => Some(syn::parse_quote!(AppEnvironment)),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &name,
                                "unknown environment value; supply its type with @Environment name: Type",
                            ));
                        }
                    };
                }
                let initialize = initialize.map(|expr| {
                    if let syn::Expr::Lit(lit) = &expr
                        && matches!(lit.lit, syn::Lit::Str(_))
                    {
                        if ty.is_some() {
                            syn::parse_quote!(::core::convert::Into::into(#expr))
                        } else {
                            syn::parse_quote!(::std::string::String::from(#expr))
                        }
                    } else {
                        expr
                    }
                });
                if environment && (ty.is_none() || initialize.is_some()) {
                    return Err(syn::Error::new_spanned(
                        &name,
                        "@Environment requires a type and no initializer",
                    ));
                }
                if responsive && (ty.is_some() || initialize.is_some()) {
                    return Err(syn::Error::new_spanned(
                        name,
                        "@Responsive takes only a binding name",
                    ));
                }
                if !environment && !responsive && initialize.is_none() {
                    return Err(syn::Error::new_spanned(
                        &name,
                        "@State requires an initializer",
                    ));
                }
                states.push(StateDecl {
                    name,
                    ty,
                    initialize,
                    environment,
                    responsive,
                    localized: false,
                    statement_index: statements.len(),
                });
                continue;
            }
            let ahead = body.fork();
            let Ok(statement) = ahead.parse::<syn::Stmt>() else {
                break;
            };
            let is_statement = match &statement {
                syn::Stmt::Local(_) | syn::Stmt::Item(_) => true,
                syn::Stmt::Expr(_, semicolon) => semicolon.is_some(),
                syn::Stmt::Macro(statement) => statement.semi_token.is_some(),
            };
            if !is_statement {
                break;
            }
            if let syn::Stmt::Macro(statement) = &statement
                && statement.mac.path.is_ident("state")
            {
                return Err(syn::Error::new_spanned(
                    &statement.mac.path,
                    "state! declares a named model; use @State let model = Model::new() inside a view",
                ));
            }
            statements.push(body.parse()?);
        }
        let tree = body.parse()?;
        Ok(Self {
            preview,
            range: start..brace.span.close().byte_range().end,
            attributes,
            visibility,
            name,
            properties,
            statements,
            states,
            tree,
        })
    }
}

pub(crate) fn bindings(tokens: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    use proc_macro2::{Group, TokenTree};
    use quote::quote_spanned;
    let mut output = proc_macro2::TokenStream::new();
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(ref punct) if punct.as_char() == '$' => {
                let Some(TokenTree::Ident(name)) = tokens.next() else {
                    return Err(syn::Error::new(
                        punct.span(),
                        "$binding requires a value name",
                    ));
                };
                output.extend(quote_spanned!(punct.span()=> __omen_binding!(#name)));
            }
            TokenTree::Group(group) => {
                let mut rewritten = Group::new(group.delimiter(), bindings(group.stream())?);
                rewritten.set_span(group.span());
                output.extend([TokenTree::Group(rewritten)]);
            }
            token => output.extend([token]),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn parses_nested_elements_and_symbolic_modifiers() {
        let tree =
            syn::parse2::<ViewTree>(quote! { Layout { Text("Hello").font(.title) } }).unwrap();
        let ViewNode::Element(root) = &tree.0[0] else {
            panic!("expected element")
        };
        let Content::Children(children) = &root.content else {
            panic!("expected children")
        };
        let ViewNode::Element(text) = &children[0] else {
            panic!("expected text")
        };
        assert_eq!(text.name, "Text");
        assert!(
            matches!(&text.modifiers[0].arguments[0], ModifierArg::Symbol(symbol) if symbol == "title")
        );
    }

    #[test]
    fn distinguishes_rust_actions_from_child_blocks() {
        let tree =
            syn::parse2::<ViewTree>(quote! { Button("Save") { let x = 1; println!("{x}"); } })
                .unwrap();
        assert!(
            matches!(&tree.0[0], ViewNode::Element(node) if matches!(node.content, Content::Action(_)))
        );
    }

    #[test]
    fn parses_token_aware_control_flow_and_list_patterns() {
        let tree = syn::parse2::<ViewTree>(quote! {
            if matches!(Some((1, 2)), Some((1, _))) && true { Text("yes") }
            else if false {} else { Text("no") }
            for (index, value) in ["a", "b"].iter().enumerate() { Text(format!("{index}: {value}")) }
            List([("a", 1), ("b", 2)]) { (name, _) in Text(name) }
        }).unwrap();
        assert_eq!(tree.0.len(), 3);
        assert!(matches!(&tree.0[1], ViewNode::Loop(_)));
        assert!(
            matches!(&tree.0[2], ViewNode::Element(node) if matches!(node.content, Content::List { .. }))
        );
    }

    #[test]
    fn rejects_empty_and_malformed_syntax() {
        for input in [
            quote!(),
            quote!(Text("a").font(.)),
            quote!(Button("a") { let = 1; }),
            quote!(List(items) { Text("a") }),
            quote!(if true Text("a")),
            quote!(Text("a"), Text("b")),
        ] {
            assert!(syn::parse2::<ViewTree>(input).is_err());
        }
    }
}
