use crate::ast::{ModifierArg, ModifierNode};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

type Symbols = (&'static str, &'static [(&'static str, &'static str)]);
struct ModifierSpec {
    method: &'static str,
    symbols: Option<Symbols>,
    arity: usize,
}
const MODES: &[(&str, &str)] = &[
    ("horizontal", "Horizontal"),
    ("vertical", "Vertical"),
    ("stack", "Stack"),
];
const FONTS: &[(&str, &str)] = &[
    ("body", "Body"),
    ("title", "Title"),
    ("largeTitle", "LargeTitle"),
    ("caption", "Caption"),
    ("footnote", "Footnote"),
    ("headline", "Headline"),
    ("title2", "Title2"),
];
const WEIGHTS: &[(&str, &str)] = &[
    ("regular", "Regular"),
    ("medium", "Medium"),
    ("bold", "Bold"),
    ("thin", "Thin"),
    ("light", "Light"),
    ("semibold", "Semibold"),
    ("heavy", "Heavy"),
];
const COLORS: &[(&str, &str)] = &[
    ("primary", "Primary"),
    ("canvas", "Canvas"),
    ("surface", "Surface"),
    ("accent", "Accent"),
    ("border", "Border"),
    ("white", "White"),
    ("black", "Black"),
    ("blue", "Blue"),
    ("secondary", "Secondary"),
    ("green", "Green"),
    ("dark", "Dark"),
    ("red", "Red"),
    ("orange", "Orange"),
    ("purple", "Purple"),
    ("gray", "Gray"),
    ("panel", "Panel"),
];
const ALIGNMENTS: &[(&str, &str)] = &[
    ("leading", "Leading"),
    ("center", "Center"),
    ("trailing", "Trailing"),
    ("top", "Top"),
    ("bottom", "Bottom"),
    ("topLeading", "TopLeading"),
    ("topTrailing", "TopTrailing"),
    ("bottomLeading", "BottomLeading"),
    ("bottomTrailing", "BottomTrailing"),
];
const THEMES: &[(&str, &str)] = &[("light", "Light"), ("dark", "Dark"), ("system", "System")];
const DIRECTIONS: &[(&str, &str)] = &[
    ("leftToRight", "LeftToRight"),
    ("rightToLeft", "RightToLeft"),
];
const SPACES: &[(&str, &str)] = &[
    ("sm", "Small"),
    ("md", "Medium"),
    ("lg", "Large"),
    ("xl", "ExtraLarge"),
    ("small", "Small"),
    ("medium", "Medium"),
    ("large", "Large"),
    ("extraLarge", "ExtraLarge"),
];
const EDGES: &[(&str, &str)] = &[
    ("all", "All"),
    ("vertical", "Vertical"),
    ("horizontal", "Horizontal"),
    ("none", "None"),
];
const TRANSITIONS: &[(&str, &str)] = &[("fade", "Fade"), ("slide", "Slide")];
const ROLES: &[(&str, &str)] = &[
    ("text", "Text"),
    ("heading", "Heading"),
    ("image", "Image"),
    ("button", "Button"),
    ("checkbox", "Checkbox"),
    ("switch", "Switch"),
    ("textField", "TextField"),
    ("progress", "Progress"),
    ("list", "List"),
    ("listItem", "ListItem"),
    ("group", "Group"),
    ("navigation", "Navigation"),
    ("tab", "Tab"),
    ("tabList", "TabList"),
    ("tabPanel", "TabPanel"),
    ("dialog", "Dialog"),
];

macro_rules! modifier_specs {
    ($($name:literal $(| $alias:literal)* => $spec:expr),+ $(,)?) => {
        pub(crate) const MODIFIER_NAMES: &[&str] = &[$($name, $($alias,)*)+];

        fn spec(name: &syn::Ident) -> syn::Result<ModifierSpec> {
            let (method, symbols, arity): (&str, Option<Symbols>, usize) =
                match name.to_string().as_str() {
                    $($name $(| $alias)* => $spec,)+
                    _ => return Err(syn::Error::new_spanned(name, "unknown modifier")),
                };
            Ok(ModifierSpec { method, symbols, arity })
        }
    };
}

modifier_specs! {
        "format" => ("format", None, 1),
        "format_context" | "formatContext" => ("format_context", None, 1),
        "context_menu" | "contextMenu" => ("context_menu", None, 2),
        "hoverBackground" => ("hover_background", Some(("Color", COLORS)), 1),
        "desktopAction" => ("desktop_action", None, 1),
        "onLongPress" => ("on_long_press", None, 1),
        "onSwipe" => ("on_swipe", None, 2),
        "theme" => ("theme", Some(("ThemeMode", THEMES)), 1),
        "locale" => ("locale", None, 1),
        "adaptive" => ("adaptive", None, 1),
        "min_item_width" | "minItemWidth" => ("min_item_width", None, 1),
        "direction" => ("direction", Some(("LayoutDirection", DIRECTIONS)), 1),
        "safeArea" => ("safe_area", Some(("SafeArea", EDGES)), 1),
        "safeAreaInsets" => ("safe_area_insets", None, 1),
        "paddingToken" => ("padding_token", Some(("Space", SPACES)), 1),
        "spacingToken" => ("spacing_token", Some(("Space", SPACES)), 1),
        "appBarVisible" => ("app_bar_visible", None, 1),
        "showBackButton" => ("show_back_button", None, 1),
        "centerTitle" => ("center_title", None, 1),
        "appBarBackground" => ("app_bar_background", Some(("Color", COLORS)), 1),
        "appBarColor" => ("app_bar_color", Some(("Color", COLORS)), 1),
        "navigationTitle" => ("navigation_title", None, 1),
        "horizontal" | "vertical" | "stack" => (
            "responsive",
            Some((
                "Breakpoint",
                &[
                    ("compact", "Compact"),
                    ("small", "Compact"),
                    ("medium", "Medium"),
                    ("large", "Large"),
                    ("extraLarge", "ExtraLarge"),
                ],
            )),
            1,
        ),
        "responsive" => ("responsive", None, 2),
        "sheet" => ("sheet", None, 2),
        "dialog" => ("dialog", None, 2),
        "popover" => ("popover", None, 2),
        "modal" => ("modal", None, 2),
        "animation" => ("animation", None, 1),
        "transition" => ("transition", Some(("Transition", TRANSITIONS)), 1),
        "opacity" => ("opacity", None, 1),
        "offset" => ("offset", None, 2),
        "reducedMotion" => ("reduced_motion", None, 1),
        "name" => ("name", None, 1),
        "key" => ("key", None, 1),
        "environment" => ("environment", None, 1),
        "onTap" => ("on_tap", None, 1),
        "onTapAsync" => ("on_tap_async", None, 1),
        "onAppear" => ("on_appear", None, 1),
        "onDisappear" => ("on_disappear", None, 1),
        "effect" => ("effect", None, 2),
        "task" => ("task", None, 1),
        "onError" => ("on_error", None, 1),
        "taskHandle" => ("task_handle", None, 1),
        "mode" => ("mode", Some(("LayoutMode", MODES)), 1),
        "font" => ("font", Some(("Font", FONTS)), 1),
        "weight" => ("weight", Some(("Weight", WEIGHTS)), 1),
        "color" | "textColor" | "textcolor" => ("color", Some(("Color", COLORS)), 1),
        "background" => ("background", Some(("Color", COLORS)), 1),
        "padding" => ("padding", None, 1),
        "spacing" => ("spacing", None, 1),
        "cornerRadius" => ("corner_radius", None, 1),
        "frame" => ("frame", None, 2),
        "fill" => ("fill", None, 0),
        "truncate" => ("truncate", None, 0),
        "document" => ("document", None, 1),
        "filePicker" => ("file_picker", None, 1),
        "window" => ("window", None, 3),
        "desktopWindow" => ("desktop_window", None, 1),
        "onWindowClose" => ("on_window_close", None, 1),
    "onWindowBlur" => ("on_window_blur", None, 1),
        "shortcut" => ("shortcut", None, 2),
        "virtualized" => ("virtualized", None, 1),
        "resizeHandle" => ("resize_handle", None, 4),
        "onContextMenu" => ("on_context_menu", None, 1),
        "scrollable" => ("scrollable", None, 0),
        "followScrollEnd" => ("follow_scroll_end", None, 0),
        "scrollableHorizontal" => ("scrollable_horizontal", None, 0),
        "minWidth" => ("min_width", None, 1),
        "pin" => (
            "pin",
            Some((
                "PinCorner",
                &[
                    ("topLeading", "TopLeading"),
                    ("topTrailing", "TopTrailing"),
                    ("bottomLeading", "BottomLeading"),
                    ("bottomTrailing", "BottomTrailing"),
                ],
            )),
            3,
        ),
        "alignment" => ("alignment", Some(("Alignment", ALIGNMENTS)), 1),
        "gradient" => ("gradient", Some(("Color", COLORS)), 3),
        "fontSize" => ("font_size", None, 1),
        "lineHeight" => ("line_height", None, 1),
        "fontFamily" => ("font_family", None, 1),
        "value" => ("value", None, 1),
        "checked" => ("checked", None, 1),
        "onChange" => ("on_change", None, 1),
        "focusable" => ("focusable", None, 1),
        "focusRequest" => ("focus_request", None, 1),
        "focusArrows" => ("focus_arrows", None, 1),
        "focusScope" => ("focus_scope", None, 1),
        "autofocus" => ("autofocus", None, 1),
        "accessibilityLabel" => ("accessibility_label", None, 1),
        "accessibilityHint" => ("accessibility_hint", None, 1),
        "accessibilityRole" => ("accessibility_role", Some(("AccessibilityRole", ROLES)), 1),
}

pub(crate) fn allowed(name: &str, component: &str) -> bool {
    match name {
        "mode" | "adaptive" => matches!(component, "Layout" | "AdaptiveLayout"),
        "min_item_width" | "minItemWidth" => component == "AdaptiveGrid",
        "spacing" => matches!(
            component,
            "Layout" | "AdaptiveLayout" | "AdaptiveGrid" | "List" | "Card"
        ),
        "value" => matches!(component, "TextField" | "TextEditor" | "SecureField"),
        "checked" => matches!(component, "Checkbox" | "Toggle"),
        "onChange" => matches!(
            component,
            "TextField" | "TextEditor" | "SecureField" | "Checkbox" | "Toggle"
        ),
        _ => true,
    }
}

pub(crate) fn completion_arguments(name: &str, component: &str) -> Option<&'static str> {
    use crate::inspector::MODIFIERS;

    if name == "adaptive" {
        return None;
    }
    if let Some(descriptor) = MODIFIERS.iter().find(|descriptor| {
        descriptor.name == name
            && (descriptor.components.is_empty() || descriptor.components.contains(&component))
    }) {
        return (!descriptor.flag).then_some(descriptor.default);
    }
    match name {
        "onTap" | "onLongPress" | "onAppear" | "onDisappear" | "onWindowClose" | "onWindowBlur"
        | "onContextMenu" | "onChange" => Some("|| {}"),
        "onTapAsync" => Some("|| async {}"),
        "focusable" | "focusArrows" | "focusScope" | "autofocus" | "reducedMotion" => Some("true"),
        "accessibilityLabel" | "accessibilityHint" | "navigationTitle" => Some(r#""""#),
        "transition" => Some(".fade"),
        "direction" => Some(".leftToRight"),
        "theme" => Some(".system"),
        "safeArea" => Some(".all"),
        "paddingToken" | "spacingToken" | "stack" => Some(".medium"),
        "minItemWidth" => Some("240"),
        "offset" => Some("0, 0"),
        "textcolor" | "textColor" => Some(".primary"),
        _ => {
            let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
            spec(&ident)
                .ok()
                .and_then(|spec| (spec.arity > 0).then_some(""))
        }
    }
}

pub(crate) fn generate(modifier: &ModifierNode, component: &str) -> syn::Result<TokenStream> {
    let layout = matches!(component, "Layout" | "AdaptiveLayout");
    if layout && modifier.name == "adaptive" && modifier.arguments.is_empty() {
        return Ok(quote!(.adaptive(::omen_ui::AdaptiveDirection::default())));
    }
    if let [ModifierArg::Adaptive(direction)] = modifier.arguments.as_slice()
        && matches!(
            modifier.name.to_string().as_str(),
            "direction" | "mode" | "adaptive"
        )
    {
        if !layout {
            return Err(syn::Error::new_spanned(
                &modifier.name,
                "adaptive direction requires Layout or AdaptiveLayout",
            ));
        }
        let variant = |mode| {
            let name = match mode {
                omen_core::LayoutMode::Vertical => "Vertical",
                omen_core::LayoutMode::Horizontal => "Horizontal",
                omen_core::LayoutMode::Stack => "Stack",
            };
            let ident = syn::Ident::new(name, modifier.span);
            quote!(::omen_ui::LayoutMode::#ident)
        };
        let compact = variant(direction.compact);
        let expanded = variant(direction.expanded);
        let medium = direction.medium.map_or_else(
            || quote!(None),
            |mode| {
                let mode = variant(mode);
                quote!(Some(#mode))
            },
        );
        return Ok(quote!(.adaptive(::omen_ui::AdaptiveDirection {
            compact: #compact, expanded: #expanded, medium: #medium,
        })));
    }
    let mut spec = spec(&modifier.name)?;
    if matches!(modifier.name.to_string().as_str(), "padding" | "spacing")
        && matches!(modifier.arguments.first(), Some(ModifierArg::Symbol(_)))
    {
        spec.symbols = Some(("Space", SPACES));
        spec.method = if modifier.name == "padding" {
            "padding_token"
        } else {
            "spacing_token"
        };
    }
    let name = modifier.name.to_string();
    if !allowed(&name, component) {
        let message = match name.as_str() {
            "mode" => "mode is only supported on Layout and AdaptiveLayout",
            "spacing" => "spacing is only supported on layout containers",
            "min_item_width" | "minItemWidth" => "minimum item width requires AdaptiveGrid",
            "value" => "value is only supported on TextField, TextEditor and SecureField",
            "checked" => "checked is only supported on Checkbox and Toggle",
            _ => "onChange is only supported on editable controls",
        };
        return Err(syn::Error::new_spanned(&modifier.name, message));
    }
    if modifier.arguments.len() != spec.arity {
        let message = if spec.arity == 1 {
            "modifier requires exactly one argument".to_string()
        } else {
            format!(
                "{} requires exactly {} arguments",
                modifier.name, spec.arity
            )
        };
        return Err(syn::Error::new_spanned(&modifier.name, message));
    }
    let arguments = modifier
        .arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| match argument {
            ModifierArg::Adaptive(_) => Err(syn::Error::new_spanned(
                &modifier.name,
                ".adaptive(...) must be used as the only direction argument",
            )),
            ModifierArg::Views(tree) => {
                crate::validation::validate(tree)?;
                // Content factories are emitted after reactive rewriting. Here
                // validation only needs to check the nested DSL and arity.
                Ok(quote!(move || ()))
            }
            ModifierArg::Rust(expr) => {
                let color_argument =
                    matches!(spec.symbols, Some(("Color", _))) && (name != "gradient" || index < 2);
                if color_argument && let syn::Expr::Lit(literal) = expr {
                    let valid = match &literal.lit {
                        syn::Lit::Str(value) => {
                            omen_core::Color::from_hex(&value.value()).map(|_| ())
                        }
                        _ => Err(omen_core::InvalidHexColor),
                    };
                    valid.map_err(|error| syn::Error::new_spanned(expr, error))?;
                }
                Ok(quote!(#expr))
            }
            ModifierArg::Action {
                block,
                asynchronous,
            } => {
                if !matches!(
                    name.as_str(),
                    "onLongPress"
                        | "onTap"
                        | "onContextMenu"
                        | "onTapAsync"
                        | "onAppear"
                        | "onDisappear"
                        | "onWindowClose"
                        | "onWindowBlur"
                        | "onChange"
                        | "task"
                ) {
                    return Err(syn::Error::new_spanned(
                        &modifier.name,
                        "this modifier does not accept an action block",
                    ));
                }
                if *asynchronous {
                    Ok(quote!(move || async move #block))
                } else {
                    Ok(quote!(move || #block))
                }
            }
            ModifierArg::Symbol(symbol) => {
                if name == "frame" && symbol == "infinity" {
                    return Ok(quote!(::core::f64::INFINITY));
                }
                if name == "format" {
                    let method = syn::Ident::new(&symbol.to_string(), symbol.span());
                    return Ok(quote_spanned!(symbol.span()=> ::omen_ui::format::FormatOption::#method()));
                }
                let domain = if name == "gradient" && index == 2 {
                    None
                } else {
                    spec.symbols
                };
                let Some((ty, symbols)) = domain else {
                    return Err(syn::Error::new_spanned(
                        symbol,
                        "expected a Rust expression, not a symbol",
                    ));
                };
                let Some((_, variant)) = symbols.iter().find(|(name, _)| symbol == name) else {
                    let choices = symbols
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(syn::Error::new_spanned(
                        symbol,
                        format!(
                            "unknown {} symbol `{symbol}`; available: {choices}",
                            modifier.name
                        ),
                    ));
                };
                let ty = syn::Ident::new(ty, symbol.span());
                let variant = syn::Ident::new(variant, symbol.span());
                Ok(quote_spanned!(symbol.span()=> ::omen_ui::#ty::#variant))
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;
    if matches!(name.as_str(), "horizontal" | "vertical" | "stack") {
        let mode = syn::Ident::new(
            match name.as_str() {
                "horizontal" => "Horizontal",
                "vertical" => "Vertical",
                _ => "Stack",
            },
            modifier.span,
        );
        return Ok(
            quote!(.responsive(#(#arguments)*, ::omen_ui::ResponsiveStyle { mode: Some(::omen_ui::LayoutMode::#mode), ..Default::default() })),
        );
    }
    let method = syn::Ident::new(spec.method, modifier.name.span());
    Ok(quote_spanned!(modifier.span=> .#method(#(#arguments),*)))
}

pub(crate) fn expression_symbols(
    name: &syn::Ident,
    tokens: TokenStream,
) -> syn::Result<TokenStream> {
    use proc_macro2::{Group, TokenTree};
    let domain = spec(name).ok().and_then(|spec| spec.symbols);
    let Some((ty, symbols)) = domain else {
        return Ok(tokens);
    };
    let input = tokens.into_iter().collect::<Vec<_>>();
    let mut output = TokenStream::new();
    let mut index = 0;
    while index < input.len() {
        if matches!(&input[index],TokenTree::Punct(p) if p.as_char()=='.')
            && (index == 0 || matches!(&input[index-1], TokenTree::Punct(p) if p.as_char()==','))
            && let Some(TokenTree::Ident(symbol)) = input.get(index + 1)
            && let Some((_, variant)) = symbols.iter().find(|(candidate, _)| symbol == candidate)
        {
            let ty = syn::Ident::new(ty, symbol.span());
            let variant = syn::Ident::new(variant, symbol.span());
            output.extend(quote!(::omen_ui::#ty::#variant));
            index += 2;
            continue;
        }
        output.extend([match &input[index] {
            TokenTree::Group(group) => {
                let mut new =
                    Group::new(group.delimiter(), expression_symbols(name, group.stream())?);
                new.set_span(group.span());
                TokenTree::Group(new)
            }
            token => token.clone(),
        }]);
        index += 1;
    }
    Ok(output)
}

/// Compiler symbol vocabulary shared by source inspection controls.
pub(crate) fn inspector_symbols(name: &str) -> Vec<&'static str> {
    if matches!(name, "pin" | "gradient") {
        return Vec::new();
    }
    let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
    let symbols = if matches!(name, "padding" | "spacing") {
        Some(("Space", SPACES))
    } else {
        spec(&ident).ok().and_then(|spec| spec.symbols)
    };
    symbols.map_or_else(Vec::new, |(_, values)| {
        values.iter().map(|(name, _)| *name).collect()
    })
}
