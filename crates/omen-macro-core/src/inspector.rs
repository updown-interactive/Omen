//! Reusable source inspector built on Omen's compiler AST. Never evaluates Rust.
use crate::ast::{Content, ModifierArg, ModifierNode, ViewNode, ViewTree};
use proc_macro2::{TokenStream, TokenTree};
use std::{borrow::Cow, collections::HashMap, ops::Range};
use syn::{Token, parse::Parser, punctuated::Punctuated, spanned::Spanned};

/// How a property is represented in source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyMode {
    /// Literal or literal argument tuple.
    Literal,
    /// Compiler theme or enumeration token.
    Token,
    /// Rust expression, binding or action.
    Expression,
}
/// Metadata for an editable native modifier. Choices come from compiler vocabulary.
#[derive(Clone, Debug)]
pub struct ModifierDescriptor {
    /// Actual DSL modifier name.
    pub name: &'static str,
    /// UI label.
    pub label: &'static str,
    /// Section in the inspector.
    pub category: &'static str,
    /// Valid initial source when adding the modifier.
    pub default: &'static str,
    /// Whether text must be quoted as a Rust string.
    pub string: bool,
    /// Whether the modifier takes no arguments.
    pub flag: bool,
    /// Component allowlist; an empty list applies to every component.
    pub components: &'static [&'static str],
}
/// Registry of supported editor controls, restricted to real Omen APIs.
pub const MODIFIERS: &[ModifierDescriptor] = &[
    ModifierDescriptor {
        name: "direction",
        label: "Adaptive direction",
        category: "Layout",
        default: ".adaptive(compact: .vertical, expanded: .horizontal)",
        string: false,
        flag: false,
        components: &["Layout", "AdaptiveLayout"],
    },
    ModifierDescriptor {
        name: "min_item_width",
        label: "Minimum item width",
        category: "Sizing",
        default: "240",
        string: false,
        flag: false,
        components: &["AdaptiveGrid"],
    },
    ModifierDescriptor {
        name: "name",
        label: "Name",
        category: "Component",
        default: "\"\"",
        string: true,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "navigationTitle",
        label: "App bar title",
        category: "Navigation",
        default: "\"\"",
        string: true,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "appBarVisible",
        label: "Show app bar",
        category: "Navigation",
        default: "true",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "showBackButton",
        label: "Show Back button",
        category: "Navigation",
        default: "true",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "centerTitle",
        label: "Center app bar title",
        category: "Navigation",
        default: "true",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "appBarBackground",
        label: "App bar background",
        category: "Navigation",
        default: ".surface",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "appBarColor",
        label: "App bar text and icon color",
        category: "Navigation",
        default: ".primary",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "mode",
        label: "Direction",
        category: "Layout",
        default: ".vertical",
        string: false,
        flag: false,
        components: &["Layout", "AdaptiveLayout"],
    },
    ModifierDescriptor {
        name: "horizontal",
        label: "Horizontal at breakpoint",
        category: "Layout",
        default: ".medium",
        string: false,
        flag: false,
        components: &["Layout"],
    },
    ModifierDescriptor {
        name: "vertical",
        label: "Vertical at breakpoint",
        category: "Layout",
        default: ".medium",
        string: false,
        flag: false,
        components: &["Layout"],
    },
    ModifierDescriptor {
        name: "pin",
        label: "Pin corner and insets",
        category: "Layout",
        default: ".topLeading, 0, 0",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "alignment",
        label: "Alignment",
        category: "Layout",
        default: ".center",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "spacing",
        label: "Spacing",
        category: "Layout",
        default: "16",
        string: false,
        flag: false,
        components: &["Layout", "AdaptiveLayout", "AdaptiveGrid", "Card", "List"],
    },
    ModifierDescriptor {
        name: "padding",
        label: "Padding",
        category: "Layout",
        default: "24",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "frame",
        label: "Width, height",
        category: "Sizing",
        default: ".infinity, .infinity",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "fill",
        label: "Fill available space",
        category: "Sizing",
        default: "",
        string: false,
        flag: true,
        components: &[],
    },
    ModifierDescriptor {
        name: "minWidth",
        label: "Minimum width",
        category: "Sizing",
        default: "100",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "font",
        label: "Font",
        category: "Typography",
        default: ".body",
        string: false,
        flag: false,
        components: &[
            "Text",
            "Button",
            "TextField",
            "TextEditor",
            "SecureField",
            "Checkbox",
            "Toggle",
        ],
    },
    ModifierDescriptor {
        name: "fontSize",
        label: "Font size",
        category: "Typography",
        default: "14",
        string: false,
        flag: false,
        components: &[
            "Text",
            "Button",
            "TextField",
            "TextEditor",
            "SecureField",
            "Checkbox",
            "Toggle",
        ],
    },
    ModifierDescriptor {
        name: "fontFamily",
        label: "Font family",
        category: "Typography",
        default: "\"monospace\"",
        string: true,
        flag: false,
        components: &["Text", "Button", "TextField", "TextEditor", "SecureField"],
    },
    ModifierDescriptor {
        name: "weight",
        label: "Weight",
        category: "Typography",
        default: ".regular",
        string: false,
        flag: false,
        components: &["Text", "Button", "TextField", "TextEditor", "SecureField"],
    },
    ModifierDescriptor {
        name: "lineHeight",
        label: "Line height",
        category: "Typography",
        default: "20",
        string: false,
        flag: false,
        components: &["Text", "TextEditor", "TextField", "Button"],
    },
    ModifierDescriptor {
        name: "color",
        label: "Text color",
        category: "Appearance",
        default: ".primary",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "background",
        label: "Background",
        category: "Appearance",
        default: ".surface",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "hoverBackground",
        label: "Hover background",
        category: "Appearance",
        default: ".surface",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "gradient",
        label: "Gradient",
        category: "Appearance",
        default: ".primary, .accent, 45",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "cornerRadius",
        label: "Corner radius",
        category: "Appearance",
        default: "6",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "opacity",
        label: "Opacity",
        category: "Appearance",
        default: "1",
        string: false,
        flag: false,
        components: &[],
    },
    ModifierDescriptor {
        name: "scrollable",
        label: "Vertical scrolling",
        category: "Scrolling",
        default: "",
        string: false,
        flag: true,
        components: &["Layout", "AdaptiveLayout", "AdaptiveGrid", "Card", "List"],
    },
    ModifierDescriptor {
        name: "scrollableHorizontal",
        label: "Horizontal scrolling",
        category: "Scrolling",
        default: "",
        string: false,
        flag: true,
        components: &["Layout", "AdaptiveLayout", "AdaptiveGrid", "Card", "List"],
    },
    ModifierDescriptor {
        name: "virtualized",
        label: "Virtual row height",
        category: "Scrolling",
        default: "28",
        string: false,
        flag: false,
        components: &["List"],
    },
];
/// One source-backed argument or modifier.
#[derive(Clone, Debug)]
pub struct Property {
    /// Stable within this syntax snapshot.
    pub key: String,
    /// Modifier or property name.
    pub name: String,
    /// User-facing label.
    pub label: String,
    /// Registry section.
    pub category: String,
    /// Exact current argument source.
    pub value: String,
    /// Editing mode.
    pub mode: PropertyMode,
    /// Editable scalar source range.
    pub value_range: Range<usize>,
    /// Full modifier range, absent for constructor properties.
    pub modifier_range: Option<Range<usize>>,
    /// Whether this property has a supported editing control.
    pub editable: bool,
    /// Available token values, in actual DSL notation.
    pub choices: Vec<String>,
    /// Explicit literal replacement default.
    pub default: String,
    /// Native no-argument flag.
    pub flag: bool,
    /// Whether the UI edits unquoted text.
    pub string: bool,
}
/// A selected source node; document ownership and version checks belong to the host.
#[derive(Clone, Debug)]
pub struct InspectedNode {
    /// Component type.
    pub component: String,
    /// Breadcrumbs from the named view to this element.
    pub path: Vec<String>,
    /// Entire node including modifier chain.
    pub range: Range<usize>,
    /// First identifier range, used to reveal the node after edits.
    pub name_range: Range<usize>,
    /// Byte offset of the closing brace of a declarative child block.
    pub children_end: Option<usize>,
    /// Existing source properties.
    pub properties: Vec<Property>,
    /// Supported modifiers not already present.
    pub available: Vec<ModifierDescriptor>,
    /// A direct button navigation to a named view with literal string arguments.
    pub preview_route: Option<omen_core::Route>,
    /// Exact current modifier source for copying.
    pub generated: String,
}

impl InspectedNode {
    /// The effective name modifier, following the last assignment in source.
    pub fn name_property(&self) -> Option<&Property> {
        self.properties
            .iter()
            .rev()
            .find(|property| property.name == "name")
    }

    /// A nonempty name Studio can display without evaluating project code.
    pub fn design_name(&self) -> Option<String> {
        let property = self.name_property()?;
        let name = syn::parse_str::<syn::LitStr>(&property.value).ok()?.value();
        (!name.trim().is_empty()).then_some(name)
    }
}

/// A precise source patch, including a check against the analyzed text.
#[derive(Clone, Debug)]
pub struct SourcePatch {
    /// Original byte range.
    pub range: Range<usize>,
    /// Expected source at the range.
    pub expected: String,
    /// Replacement source.
    pub replacement: String,
}
#[derive(Clone)]
struct Extent {
    range: Range<usize>,
    args: Option<Range<usize>>,
    tokens: Vec<TokenTree>,
    body_end: Option<usize>,
}
fn extents(tokens: TokenStream, result: &mut HashMap<usize, Extent>) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        let start = token.span().byte_range().start;
        if matches!(token, TokenTree::Ident(_)) {
            let mut end = token.span().byte_range().end;
            let mut arguments = Vec::new();
            let mut body_end = None;
            let mut next = index + 1;
            if let Some(TokenTree::Group(group)) = tokens.get(next)
                && group.delimiter() == proc_macro2::Delimiter::Parenthesis
            {
                end = group.span().byte_range().end;
                arguments = group.stream().into_iter().collect();
                next += 1;
            }
            while matches!(tokens.get(next),Some(TokenTree::Ident(name)) if name=="async" || name=="move")
            {
                next += 1;
            }
            if let Some(TokenTree::Group(group)) = tokens.get(next)
                && group.delimiter() == proc_macro2::Delimiter::Brace
            {
                end = group.span().byte_range().end;
                body_end = Some(group.span_close().byte_range().start);
            }
            result.insert(
                start,
                Extent {
                    range: start..end,
                    args: None,
                    tokens: arguments,
                    body_end,
                },
            );
        } else if matches!(token,TokenTree::Punct(p) if p.as_char()=='.')
            && matches!(tokens.get(index + 1), Some(TokenTree::Ident(_)))
            && let Some(TokenTree::Group(group)) = tokens.get(index + 2)
        {
            result.insert(
                start,
                Extent {
                    range: start..group.span().byte_range().end,
                    args: Some(
                        group.span_open().byte_range().end..group.span_close().byte_range().start,
                    ),
                    tokens: group.stream().into_iter().collect(),
                    body_end: None,
                },
            );
        }
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            extents(group.stream(), result);
        }
    }
}
/// Inspect the innermost Omen node at a UTF-8 cursor offset.
/// Invalid/incomplete source returns an error instead of guessing edit ranges.
pub fn inspect(source: &str, cursor: usize) -> Result<Option<InspectedNode>, String> {
    let tokens = source
        .parse::<TokenStream>()
        .map_err(|error| format!("Finish the source syntax to inspect it: {error}"))?;
    let mut index = HashMap::new();
    extents(tokens.clone(), &mut index);
    let mut nodes = Vec::new();
    macros(tokens, source, &index, &mut nodes);
    nodes.sort_by_key(|node: &InspectedNode| {
        (
            if node.name_range.contains(&cursor) {
                0
            } else {
                1
            },
            node.range.len(),
        )
    });
    Ok(nodes
        .into_iter()
        .find(|node| node.range.start <= cursor && cursor <= node.range.end))
}
/// Analyze one top-level named view whose hierarchy is declared statically.
pub fn visual_nodes(source: &str) -> Result<Vec<InspectedNode>, String> {
    let mut declarations = visual_declarations(source)?.into_iter();
    let first = declarations
        .next()
        .ok_or("Design requires a top-level named view! declaration.")?;
    if declarations.next().is_some() {
        return Err("Select a declaration in files containing multiple views.".into());
    }
    first.nodes
}

/// Source range and static preview analysis for one top-level view declaration.
pub struct VisualDeclaration {
    /// Full range of the declaration in the source document.
    pub range: Range<usize>,
    /// Whether this declaration is the Studio default.
    pub preview: bool,
    /// Names of the properties required to instantiate the view.
    pub parameters: Vec<String>,
    /// Inspectable static view nodes, or a diagnostic for this declaration.
    pub nodes: Result<Vec<InspectedNode>, String>,
}

/// End byte of the first design marker directly inside a top-level `view!`.
///
/// This checks the declaration tokens independently of errors inside the view body.
pub fn preview_marker_end(source: &str) -> Result<Option<usize>, proc_macro2::LexError> {
    if !source.contains("Design") {
        return Ok(None);
    }
    let mut tokens = source.parse::<TokenStream>()?.into_iter().peekable();
    while let Some(token) = tokens.next() {
        if !matches!(token, TokenTree::Ident(name) if name == "view")
            || !matches!(tokens.peek(), Some(TokenTree::Punct(bang)) if bang.as_char() == '!')
        {
            continue;
        }
        tokens.next();
        let Some(TokenTree::Group(group)) = tokens.next() else {
            continue;
        };
        let mut body = group.stream().into_iter().peekable();
        while let Some(token) = body.next() {
            if matches!(token, TokenTree::Punct(at) if at.as_char() == '@')
                && let Some(TokenTree::Ident(marker)) = body.peek()
                && crate::parser::is_design_marker(marker)
            {
                return Ok(Some(marker.span().byte_range().end));
            }
        }
    }
    Ok(None)
}

/// Analyze each top-level Omen view independently with document-relative ranges.
pub fn visual_declarations(source: &str) -> Result<Vec<VisualDeclaration>, String> {
    let file = syn::parse_file(source).map_err(|error| format!("Rust syntax: {error}"))?;
    let mut declarations = Vec::new();
    for item in file.items {
        let syn::Item::Macro(item) = item else {
            continue;
        };
        if item
            .mac
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "view")
        {
            continue;
        }
        let tokens = item.mac.tokens.clone();
        let named = crate::sugar::actions(tokens.clone())
            .and_then(crate::parser::bindings)
            .and_then(syn::parse2::<crate::ast::NamedViews>);
        match named {
            Ok(views) => {
                let mut index = HashMap::new();
                extents(tokens, &mut index);
                for (ordinal, declaration) in views.0.into_iter().enumerate() {
                    let mut range = declaration.range.clone();
                    if ordinal == 0 {
                        range.start = item.span().byte_range().start;
                    }
                    declarations.push(VisualDeclaration {
                        range,
                        preview: declaration.preview.is_some(),
                        parameters: declaration
                            .properties
                            .iter()
                            .map(|property| property.name.to_string())
                            .collect(),
                        nodes: visual_declaration_nodes(source, &index, &declaration),
                    });
                }
            }
            Err(error) => declarations.push(VisualDeclaration {
                range: item.span().byte_range(),
                preview: false,
                parameters: Vec::new(),
                nodes: Err(format!("Omen syntax: {error}")),
            }),
        }
    }
    if declarations
        .iter()
        .filter(|declaration| declaration.preview)
        .count()
        > 1
    {
        return Err("Only one view per file may be marked @Design.".into());
    }
    Ok(declarations)
}

fn preview_route(content: &Content) -> Option<omen_core::Route> {
    use syn::{Expr, Lit, Stmt};

    let Content::Action(block) = content else {
        return None;
    };
    let [Stmt::Expr(Expr::MethodCall(call), _)] = block.stmts.as_slice() else {
        return None;
    };
    if !matches!(&*call.receiver, Expr::Path(path) if path.path.get_ident().is_some()) {
        return None;
    }
    let argument = call.args.first().filter(|_| call.args.len() == 1)?;
    let tree = match argument {
        Expr::Macro(view)
            if (call.method == "push_view" || call.method == "push")
                && view
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "view") =>
        {
            syn::parse2::<ViewTree>(view.mac.tokens.clone()).ok()?
        }
        _ => return None,
    };
    let [ViewNode::Element(element)] = tree.0.as_slice() else {
        return None;
    };
    if !matches!(element.content, Content::None) || !element.modifiers.is_empty() {
        return None;
    }
    let mut route = omen_core::Route::new(element.name.to_string());
    for argument in &element.arguments {
        let name = argument.name.as_ref()?.to_string();
        let Expr::Lit(literal) = &argument.value else {
            return None;
        };
        let Lit::Str(value) = &literal.lit else {
            return None;
        };
        if route.parameters.insert(name, value.value()).is_some() {
            return None;
        }
    }
    Some(route)
}

fn visual_declaration_nodes(
    source: &str,
    index: &HashMap<usize, Extent>,
    declaration: &crate::ast::NamedView,
) -> Result<Vec<InspectedNode>, String> {
    crate::validation::validate(&declaration.tree).map_err(|error| error.to_string())?;
    validate_static_tree(&declaration.tree.0)?;
    let mut nodes = Vec::new();
    visit(
        &declaration.tree.0,
        source,
        index,
        &[declaration.name.to_string()],
        &mut nodes,
    );
    if nodes
        .iter()
        .any(|node| source.get(node.name_range.clone()) != Some(node.component.as_str()))
    {
        return Err("Unable to resolve component source ranges.".into());
    }
    if nodes.is_empty() {
        return Err("Add a component in Code to start designing this view.".into());
    }
    Ok(nodes)
}

fn validate_static_tree(nodes: &[ViewNode]) -> Result<(), String> {
    for node in nodes {
        let ViewNode::Element(element) = node else {
            return Err("Conditional and repeated views require a project runtime.".into());
        };
        match &element.content {
            Content::Children(children) => validate_static_tree(children)?,
            Content::List { .. } => return Err("Dynamic lists require a project runtime.".into()),
            _ => {}
        }
        if element.modifiers.iter().any(|modifier| {
            modifier
                .arguments
                .iter()
                .any(|arg| matches!(arg, ModifierArg::Views(_)))
        }) {
            return Err("View-valued modifiers require a project runtime.".into());
        }
    }
    Ok(())
}

fn macros(
    tokens: TokenStream,
    source: &str,
    index: &HashMap<usize, Extent>,
    output: &mut Vec<InspectedNode>,
) {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for window in tokens.windows(3) {
        if matches!(&window[0],TokenTree::Ident(name) if name=="view" || name=="main")
            && matches!(&window[1],TokenTree::Punct(p) if p.as_char()=='!')
            && let TokenTree::Group(group) = &window[2]
            && (matches!(&window[0], TokenTree::Ident(name) if name == "view")
                || matches!(group.stream().into_iter().next(),
                    Some(TokenTree::Ident(name)) if name.to_string().starts_with(char::is_uppercase)))
            && let Ok(tokens) =
                crate::sugar::actions(group.stream()).and_then(crate::parser::bindings)
        {
            if crate::is_view_declaration(tokens.clone()) {
                if let Ok(views) = syn::parse2::<crate::ast::NamedViews>(tokens) {
                    for view in views.0 {
                        visit(
                            &view.tree.0,
                            source,
                            index,
                            &[view.name.to_string()],
                            output,
                        );
                    }
                }
            } else if let Ok(tree) = syn::parse2::<ViewTree>(tokens) {
                visit(&tree.0, source, index, &[], output);
            }
        }
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            macros(group.stream(), source, index, output);
        }
    }
}
fn numeric(expression: &syn::Expr) -> bool {
    matches!(expression,syn::Expr::Lit(lit) if matches!(lit.lit,syn::Lit::Int(_)|syn::Lit::Float(_)))
        || matches!(expression,syn::Expr::Unary(unary) if matches!(unary.op,syn::UnOp::Neg(_)) && numeric(&unary.expr))
}
fn classify(value: &str) -> PropertyMode {
    let Ok(arguments) = Punctuated::<ModifierArg, Token![,]>::parse_terminated.parse_str(value)
    else {
        return PropertyMode::Expression;
    };
    if arguments.is_empty() {
        return PropertyMode::Expression;
    }
    let mut token = false;
    for argument in arguments {
        match argument {
            ModifierArg::Symbol(_) | ModifierArg::Adaptive(_) => token = true,
            ModifierArg::Rust(expression)
                if matches!(expression, syn::Expr::Lit(_)) || numeric(&expression) => {}
            _ => return PropertyMode::Expression,
        }
    }
    if token {
        PropertyMode::Token
    } else {
        PropertyMode::Literal
    }
}
fn visit(
    nodes: &[ViewNode],
    source: &str,
    index: &HashMap<usize, Extent>,
    parents: &[String],
    output: &mut Vec<InspectedNode>,
) {
    for node in nodes {
        match node {
            ViewNode::Element(element) => {
                let name_range = element.name.span().byte_range();
                let Some(base) = index.get(&name_range.start) else {
                    continue;
                };
                let component = element.name.to_string();
                let mut path = parents.to_vec();
                path.push(component.clone());
                let mut end = base.range.end;
                let mut properties = Vec::new();
                let mut generated = Vec::new();
                let mut raw_arguments: Vec<Vec<TokenTree>> = vec![Vec::new()];
                for token in &base.tokens {
                    if matches!(token, TokenTree::Punct(p) if p.as_char()==',') {
                        raw_arguments.push(Vec::new());
                    } else {
                        if let Some(group) = raw_arguments.last_mut() {
                            group.push(token.clone());
                        }
                    }
                }
                for (i, arg) in element.arguments.iter().enumerate() {
                    let original = raw_arguments.get(i).map(Vec::as_slice).unwrap_or(&[]);
                    let original = if arg.name.is_some() && original.len() >= 2 {
                        &original[2..]
                    } else {
                        original
                    };
                    let range = original.first().zip(original.last()).map_or_else(
                        || arg.value.span().byte_range(),
                        |(first, last)| {
                            first.span().byte_range().start..last.span().byte_range().end
                        },
                    );
                    if range.end > source.len() {
                        continue;
                    }
                    let value = source[range.clone()].to_string();
                    let mode = classify(&value);
                    let label = arg
                        .name
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| {
                            if i == 0 {
                                match component.as_str() {
                                    "Text" | "Button" => "Label",
                                    "TextField" | "SecureField" | "TextEditor" => "Placeholder",
                                    "Image" => "Image source",
                                    "Gap" => "Size",
                                    "Icon" => "Icon",
                                    _ => "Argument",
                                }
                                .into()
                            } else {
                                format!("Argument {}", i + 1)
                            }
                        });
                    properties.push(Property {
                        key: format!("arg:{i}"),
                        name: label.clone(),
                        label,
                        category: "Component".into(),
                        value_range: range,
                        value: value.clone(),
                        mode,
                        modifier_range: None,
                        editable: mode == PropertyMode::Literal || matches!(component.as_str(), "Text"|"Button"|"TextField"|"SecureField"|"TextEditor"|"Image"|"Icon"|"NavigationBar"|"Tab"),
                        choices: Vec::new(),
                        string: matches!(syn::parse_str::<syn::Expr>(&value),Ok(syn::Expr::Lit(lit)) if matches!(lit.lit,syn::Lit::Str(_))) || matches!(component.as_str(),"Text"|"Button"|"TextField"|"SecureField"|"TextEditor"|"Image"|"Icon"|"NavigationBar"|"Tab"),
                        default: if mode==PropertyMode::Expression { "\"Text\"".into() } else {value},
                        flag: false,
                    });
                }
                for modifier in &element.modifiers {
                    let Some(extent) = index.get(&modifier.span.byte_range().start) else {
                        continue;
                    };
                    if source.as_bytes().get(extent.range.start) != Some(&b'.') {
                        continue;
                    }
                    end = end.max(extent.range.end);
                    let Some(args) = &extent.args else {
                        continue;
                    };
                    let value_range = extent
                        .tokens
                        .first()
                        .zip(extent.tokens.last())
                        .map(|(first, last)| {
                            first.span().byte_range().start..last.span().byte_range().end
                        })
                        .unwrap_or(args.clone());
                    if extent.range.end > source.len() {
                        continue;
                    }
                    let name = modifier.name.to_string();
                    let descriptor = MODIFIERS.iter().find(|item| {
                        item.name == canonical_modifier_name(&name)
                            && (item.components.is_empty()
                                || item.components.contains(&component.as_str()))
                    });
                    let value = source[value_range.clone()].to_string();
                    let mode = if descriptor.is_some_and(|d| d.flag) {
                        PropertyMode::Literal
                    } else {
                        classify(&value)
                    };
                    generated.push(source[extent.range.clone()].to_string());
                    properties.push(Property {
                        key: format!("modifier:{}", extent.range.start),
                        label: descriptor.map_or_else(|| name.clone(), |d| d.label.into()),
                        category: descriptor.map_or("Code", |d| d.category).into(),
                        name: name.clone(),
                        value_range,
                        modifier_range: Some(extent.range.clone()),
                        value,
                        mode,
                        editable: descriptor.is_some(),
                        choices: crate::modifiers::inspector_symbols(&name)
                            .into_iter()
                            .map(|name| format!(".{name}"))
                            .collect(),
                        default: descriptor.map_or("", |d| d.default).into(),
                        flag: descriptor.is_some_and(|d| d.flag),
                        string: descriptor.is_some_and(|d| d.string),
                    });
                }
                let available = MODIFIERS
                    .iter()
                    .filter(|item| {
                        (item.components.is_empty()
                            || item.components.contains(&component.as_str()))
                            && !properties.iter().any(|property| {
                                property.modifier_range.is_some()
                                    && canonical_modifier_name(&property.name) == item.name
                            })
                    })
                    .cloned()
                    .collect();
                output.push(InspectedNode {
                    component,
                    path: path.clone(),
                    range: name_range.start..end,
                    name_range,
                    children_end: if matches!(element.content, Content::Children(_)) {
                        base.body_end
                    } else {
                        None
                    },
                    properties,
                    available,
                    preview_route: preview_route(&element.content),
                    generated: generated.join("\n"),
                });
                match &element.content {
                    Content::Children(nodes) | Content::List { body: nodes, .. } => {
                        visit(nodes, source, index, &path, output)
                    }
                    _ => {}
                }
                for modifier in &element.modifiers {
                    for arg in &modifier.arguments {
                        if let ModifierArg::Views(tree) = arg {
                            visit(&tree.0, source, index, &path, output);
                        }
                    }
                }
            }
            ViewNode::Conditional(node) => {
                visit(&node.then_branch, source, index, parents, output);
                visit(&node.else_branch, source, index, parents, output);
            }
            ViewNode::Loop(node) => visit(&node.body, source, index, parents, output),
        }
    }
}
fn validate(name: &str, value: &str, component: &str) -> Result<(), String> {
    if name == "name" && syn::parse_str::<syn::LitStr>(value).is_err() {
        return Err("Enter a text name.".into());
    }
    if [
        "padding",
        "spacing",
        "fontSize",
        "lineHeight",
        "cornerRadius",
        "minWidth",
        "min_item_width",
        "minItemWidth",
        "virtualized",
        "opacity",
    ]
    .contains(&name)
        && !value.trim().starts_with('.')
        && !syn::parse_str::<syn::Expr>(value).is_ok_and(|expression| numeric(&expression))
    {
        return Err("Enter a numeric value or a supported spacing token".into());
    }
    if name == "pin" {
        let values: Vec<_> = value
            .trim_end_matches(',')
            .split(',')
            .map(str::trim)
            .collect();
        if values.len() != 3 || !values[1..].iter().all(|value| value.parse::<u16>().is_ok()) {
            return Err("Enter a corner token and two whole-number insets.".into());
        }
    }
    let arguments = Punctuated::<ModifierArg, Token![,]>::parse_terminated
        .parse_str(value)
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    crate::modifiers::generate(
        &ModifierNode {
            name: syn::parse_str(name).map_err(|e: syn::Error| e.to_string())?,
            arguments,
            span: proc_macro2::Span::call_site(),
        },
        component,
    )
    .map_err(|e| e.to_string())?;
    if [
        "padding",
        "spacing",
        "fontSize",
        "lineHeight",
        "cornerRadius",
        "minWidth",
        "min_item_width",
        "minItemWidth",
        "virtualized",
        "opacity",
    ]
    .contains(&name)
        && let Ok(number) = value.parse::<f64>()
        && (!number.is_finite()
            || number < 0.
            || name == "opacity" && number > 1.
            || ["fontSize", "lineHeight", "virtualized"].contains(&name) && number == 0.)
    {
        return Err("Value is outside the supported range".into());
    }
    Ok(())
}
fn canonical_modifier_name(name: &str) -> &str {
    match name {
        "textColor" | "textcolor" => "color",
        _ => name,
    }
}

/// Whether the property accepts a single color token or hex color.
pub fn is_color_modifier(name: &str) -> bool {
    matches!(
        canonical_modifier_name(name),
        "color" | "background" | "hoverBackground"
    )
}

fn quote_hex(value: &str) -> Cow<'_, str> {
    let trimmed = value.trim();
    if omen_core::Color::from_hex(trimmed).is_ok() {
        Cow::Owned(format!("{trimmed:?}"))
    } else {
        Cow::Borrowed(value)
    }
}

fn normalize_color_input<'a>(name: &str, value: &'a str) -> Cow<'a, str> {
    if is_color_modifier(name) {
        return quote_hex(value);
    }
    if name == "gradient" {
        let mut parts = value.splitn(3, ',');
        if let (Some(from), Some(to), Some(angle)) = (parts.next(), parts.next(), parts.next()) {
            let from = quote_hex(from);
            let to = quote_hex(to);
            if matches!(from, Cow::Owned(_)) || matches!(to, Cow::Owned(_)) {
                return Cow::Owned(format!("{}, {}, {}", from.trim(), to.trim(), angle.trim()));
            }
        }
    }
    Cow::Borrowed(value)
}

/// Change a literal/token property. Expressions require an explicit opt-in.
pub fn set_property(
    source: &str,
    node: &InspectedNode,
    key: &str,
    value: &str,
    replace_expression: bool,
) -> Result<SourcePatch, String> {
    let property = node
        .properties
        .iter()
        .find(|property| property.key == key)
        .ok_or("Property no longer exists")?;
    if !property.editable {
        return Err("Edit this property in the code editor".into());
    }
    if property.mode == PropertyMode::Expression && !replace_expression {
        return Err(
            "This value is controlled by a Rust expression. Explicitly replace it first.".into(),
        );
    }
    let normalized = if property.modifier_range.is_some() {
        normalize_color_input(&property.name, value)
    } else {
        Cow::Borrowed(value)
    };
    let value = normalized.as_ref();
    if property.modifier_range.is_some() {
        validate(&property.name, value, &node.component)?;
        if classify(value) == PropertyMode::Expression {
            return Err("Use the code editor to enter Rust expressions".into());
        }
    } else if property.string && !syn::parse_str::<syn::LitStr>(value).is_ok() {
        return Err("A string literal is required".into());
    } else if !matches!(
        syn::parse_str::<syn::Expr>(value),
        Ok(syn::Expr::Lit(_) | syn::Expr::Unary(_))
    ) {
        return Err("A literal Rust value is required".into());
    }
    if property.modifier_range.is_none() && property.mode == PropertyMode::Literal {
        let old = syn::parse_str::<syn::Expr>(&property.value).map_err(|e| e.to_string())?;
        let new = syn::parse_str::<syn::Expr>(value).map_err(|e| e.to_string())?;
        if matches!(&old,syn::Expr::Lit(lit) if matches!(lit.lit,syn::Lit::Bool(_)))
            && !matches!(&new,syn::Expr::Lit(lit) if matches!(lit.lit,syn::Lit::Bool(_)))
        {
            return Err("A boolean value is required".into());
        }
        if numeric(&old) && !numeric(&new) {
            return Err("A numeric value is required".into());
        }
    }
    // Replacing a theme token only changes its identifier, retaining comments
    // between the leading dot and the token name.
    if property.mode == PropertyMode::Token
        && property.choices.iter().any(|choice| choice == value)
        && let Ok(tokens) = property.value.parse::<TokenStream>()
    {
        let tokens: Vec<_> = tokens.into_iter().collect();
        if let [TokenTree::Punct(dot), TokenTree::Ident(ident)] = tokens.as_slice()
            && dot.as_char() == '.'
        {
            let local = ident.span().byte_range();
            let range =
                property.value_range.start + local.start..property.value_range.start + local.end;
            return Ok(SourcePatch {
                expected: source[range.clone()].to_string(),
                range,
                replacement: value.trim_start_matches('.').to_string(),
            });
        }
    }
    let expected = source
        .get(property.value_range.clone())
        .ok_or("Stale property range")?
        .to_string();
    Ok(SourcePatch {
        range: property.value_range.clone(),
        expected,
        replacement: value.to_string(),
    })
}
/// Extend a removal to adjacent blank lines when the syntax owns its lines.
pub fn removal_range(source: &str, range: Range<usize>) -> Range<usize> {
    let Some(prefix) = source.get(..range.start) else {
        return range;
    };
    let Some(suffix) = source.get(range.end..) else {
        return range;
    };
    let leading = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
    let trailing = suffix.split_inclusive('\n').next().unwrap_or_default();
    if !leading.trim().is_empty() || !trailing.trim().is_empty() {
        return range;
    }

    let mut start = range.start - leading.len();
    while start > 0 {
        let previous = &source[..start - 1];
        let line = previous
            .rsplit_once('\n')
            .map_or(previous, |(_, line)| line);
        if !line.trim().is_empty() {
            break;
        }
        start -= line.len() + 1;
    }
    let end = range.end + trailing.len();
    let after = source[end..]
        .split_inclusive('\n')
        .take_while(|line| line.trim().is_empty())
        .map(str::len)
        .sum::<usize>();
    start..end + after
}

/// Explicitly remove an existing modifier without touching its children or neighbors.
pub fn remove_modifier(
    source: &str,
    node: &InspectedNode,
    key: &str,
) -> Result<SourcePatch, String> {
    let property = node
        .properties
        .iter()
        .find(|property| property.key == key)
        .ok_or("Property no longer exists")?;
    let range = property
        .modifier_range
        .clone()
        .ok_or("Constructor arguments cannot be removed here")?;
    let range = removal_range(source, range);
    Ok(SourcePatch {
        expected: source
            .get(range.clone())
            .ok_or("Stale modifier range")?
            .into(),
        range,
        replacement: String::new(),
    })
}
/// Append a registry modifier using actual compiler-validated syntax.
pub fn add_modifier(source: &str, node: &InspectedNode, name: &str) -> Result<SourcePatch, String> {
    let descriptor = node
        .available
        .iter()
        .find(|descriptor| descriptor.name == name)
        .ok_or("Modifier is not available on this component")?;
    insert_modifier(source, node, name, descriptor.default)
}

/// Effective source modifier controlling vertical, horizontal, stack or adaptive arrangement.
pub fn layout_direction_property(node: &InspectedNode) -> Option<&Property> {
    node.properties.iter().rev().find(|property| {
        property.name == "mode"
            || property.name == "adaptive"
            || (property.name == "direction" && adaptive_direction(&property.value).is_some())
    })
}

/// Update the arrangement of a layout, retaining unrelated modifiers.
pub fn set_layout_direction(
    source: &str,
    node: &InspectedNode,
    value: &str,
) -> Result<SourcePatch, String> {
    if !matches!(node.component.as_str(), "Layout" | "AdaptiveLayout") {
        return Err("Select a Layout or AdaptiveLayout.".into());
    }
    let name = if adaptive_direction(value).is_some() {
        "direction"
    } else {
        "mode"
    };
    validate(name, value, &node.component)?;
    if let Some(property) = layout_direction_property(node) {
        if property.name == name {
            return set_property(source, node, &property.key, value, false);
        }
        if property.mode == PropertyMode::Expression
            && !(property.name == "adaptive" && property.value.is_empty())
        {
            return Err("Edit the direction expression in source.".into());
        }
        let range = property
            .modifier_range
            .clone()
            .ok_or("Missing modifier range")?;
        return Ok(SourcePatch {
            expected: source
                .get(range.clone())
                .ok_or("Stale modifier range")?
                .into(),
            range,
            replacement: format!(".{name}({value})"),
        });
    }
    insert_modifier(source, node, name, value)
}

/// Rename an element, inserting its name modifier on the first edit.
pub fn set_name(source: &str, node: &InspectedNode, name: &str) -> Result<SourcePatch, String> {
    let value = format!("{name:?}");
    if let Some(property) = node.name_property() {
        return set_property(source, node, &property.key, &value, false);
    }
    insert_modifier(source, node, "name", &value)
}

fn insert_modifier(
    source: &str,
    node: &InspectedNode,
    name: &str,
    value: &str,
) -> Result<SourcePatch, String> {
    validate(name, value, &node.component)?;
    let mut end = node.range.end;
    let tail = source.get(end..).ok_or("Stale node range")?;
    let line = tail.split('\n').next().unwrap_or_default();
    if line.trim_start().starts_with("//") {
        end += line.len();
    }
    let prefix = &source[..node.name_range.start];
    let indent = prefix
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    Ok(SourcePatch {
        range: end..end,
        expected: String::new(),
        replacement: format!("{newline}{indent}.{name}({value})"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patched(source: &str, patch: SourcePatch) -> String {
        assert_eq!(&source[patch.range.clone()], patch.expected);
        let mut result = source.to_string();
        result.replace_range(patch.range, &patch.replacement);
        result
    }
    #[test]
    fn color_fields_write_valid_hex_literals_and_keep_aliases() {
        let source = "view! { Home { Text(\"Sample\").textcolor(.primary).gradient(.primary, .accent, 45) } }";
        let node = inspect(source, source.find("Text").unwrap())
            .unwrap()
            .unwrap();
        let text_color = node
            .properties
            .iter()
            .find(|property| property.name == "textcolor")
            .unwrap();
        assert!(text_color.editable);
        assert!(
            !node
                .available
                .iter()
                .any(|modifier| modifier.name == "color")
        );
        let result = patched(
            source,
            set_property(source, &node, &text_color.key, "#2f80ED", false).unwrap(),
        );
        assert!(result.contains(r##".textcolor("#2f80ED")"##));
        assert!(set_property(source, &node, &text_color.key, r##""#oops""##, false).is_err());

        let gradient = node
            .properties
            .iter()
            .find(|property| property.name == "gradient")
            .unwrap();
        let result = patched(
            source,
            set_property(source, &node, &gradient.key, ".accent, #0123, 45", false).unwrap(),
        );
        assert!(result.contains(r##".gradient(.accent, "#0123", 45)"##));
    }

    #[test]
    fn selects_nested_nodes_and_preserves_comments_and_children_when_editing() {
        let source = "view! { Home { Layout { // children\n Text(\"مرحباً\") } .spacing(.lg).padding(/* keep */24) } }";
        let layout = inspect(source, source.find("Layout").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(layout.component, "Layout");
        assert_eq!(layout.path, vec!["Home", "Layout"]);
        let property = layout
            .properties
            .iter()
            .find(|property| property.name == "padding")
            .unwrap();
        assert_eq!(property.value, "24");
        let result = patched(
            source,
            set_property(source, &layout, &property.key, "32", false).unwrap(),
        );
        assert_eq!(result, source.replace("/* keep */24", "/* keep */32"));
        let text = inspect(source, source.find("مرحباً").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(text.component, "Text");
        assert!(!text.available.iter().any(|item| item.name == "mode"));
    }
    #[test]
    fn bound_expressions_are_read_only_until_explicit_replacement() {
        for expression in ["state.padding", "-state.padding", ".infinity, state.height"] {
            let name = if expression.contains(',') {
                "frame"
            } else {
                "padding"
            };
            let source = format!("view! {{ Home {{ Layout {{}}.{name}({expression}) }} }}");
            let node = inspect(&source, source.find("Layout").unwrap())
                .unwrap()
                .unwrap();
            let property = &node.properties[0];
            assert_eq!(property.mode, PropertyMode::Expression);
            assert!(set_property(&source, &node, &property.key, "32", false).is_err());
            let value = if name == "frame" {
                "32, .infinity"
            } else {
                "32"
            };
            assert!(set_property(&source, &node, &property.key, value, true).is_ok());
        }
    }
    #[test]
    fn adds_and_removes_real_modifiers_without_reformatting_neighbors() {
        let source = "view! { Home { Layout {}.padding(24) // keep\n } }";
        let node = inspect(source, source.find("Layout").unwrap())
            .unwrap()
            .unwrap();
        let added = patched(source, add_modifier(source, &node, "background").unwrap());
        assert!(added.contains(".padding(24) // keep\n.background(.surface)"));
        let property = &node.properties[0];
        let removed = patched(
            source,
            remove_modifier(source, &node, &property.key).unwrap(),
        );
        assert_eq!(removed, "view! { Home { Layout {} // keep\n } }");
        assert!(add_modifier(source, &node, "padding").is_err());
    }
    #[test]
    fn validates_control_values_and_keeps_bound_constructor_arguments_intact() {
        let source = "view! { Home { Text(strings.welcome).fontSize(14) } }";
        let node = inspect(source, source.find("Text").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(node.properties[0].value, "strings.welcome");
        assert_eq!(node.properties[0].mode, PropertyMode::Expression);
        let size = node
            .properties
            .iter()
            .find(|p| p.name == "fontSize")
            .unwrap();
        assert!(set_property(source, &node, &size.key, "\"oops\"", false).is_err());
        assert!(set_property(source, &node, &size.key, "-2", false).is_err());
    }
}

/// Rebase cached properties after a validated source patch until background parsing completes.
pub fn rebase(node: &mut InspectedNode, patch: &SourcePatch, source: &str) {
    fn shift(range: &mut Range<usize>, patch: &SourcePatch) {
        let delta = patch.replacement.len() as isize - patch.range.len() as isize;
        if range.start >= patch.range.end {
            range.start = range.start.saturating_add_signed(delta);
        }
        if range.end >= patch.range.end {
            range.end = range.end.saturating_add_signed(delta);
        }
    }
    node.properties.retain(|p| {
        !p.modifier_range
            .as_ref()
            .is_some_and(|range| patch.range.start <= range.start && range.end <= patch.range.end)
    });
    shift(&mut node.range, patch);
    shift(&mut node.name_range, patch);
    if let Some(end) = node.children_end {
        let mut offset = end..end;
        shift(&mut offset, patch);
        node.children_end = Some(offset.start);
    }
    for property in &mut node.properties {
        shift(&mut property.value_range, patch);
        if let Some(range) = &mut property.modifier_range {
            shift(range, patch);
        }
        if let Some(value) = source.get(property.value_range.clone()) {
            property.value = value.to_string();
            property.mode = classify(value);
        }
    }
}

/// Read the same adaptive direction syntax used by the compiler from inspector source.
pub fn adaptive_direction(value: &str) -> Option<omen_core::AdaptiveDirection> {
    match syn::parse_str::<ModifierArg>(value).ok()? {
        ModifierArg::Adaptive(direction) => Some(direction),
        _ => None,
    }
}
