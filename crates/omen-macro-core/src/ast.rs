//! Compiler syntax tree. This is never used as the runtime UI model.
use proc_macro2::Span;
use syn::{Block, Expr, Ident, Pat, spanned::Spanned};

pub(crate) struct ViewTree(pub Vec<ViewNode>);
pub(crate) enum ViewNode {
    Element(Box<ElementNode>),
    Conditional(Box<ConditionalNode>),
    Loop(Box<LoopNode>),
}
pub(crate) struct ElementNode {
    pub name: Ident,
    pub arguments: Vec<Argument>,
    pub paren_span: Option<proc_macro2::extra::DelimSpan>,
    pub content: Content,
    pub modifiers: Vec<ModifierNode>,
    pub span: Span,
}
pub(crate) enum Content {
    None,
    Children(Vec<ViewNode>),
    Action(Block),
    AsyncAction(Block),
    List {
        binding: Box<Pat>,
        body: Vec<ViewNode>,
    },
}
pub(crate) struct ModifierNode {
    pub name: Ident,
    pub arguments: Vec<ModifierArg>,
    pub span: Span,
}
pub(crate) enum ModifierArg {
    Views(ViewTree),
    Rust(Expr),
    Symbol(Ident),
    Adaptive(omen_core::AdaptiveDirection),
    Action { block: Block, asynchronous: bool },
}
pub(crate) struct ConditionalNode {
    pub condition: Expr,
    pub then_branch: Vec<ViewNode>,
    pub else_branch: Vec<ViewNode>,
    pub span: Span,
}
pub(crate) struct LoopNode {
    pub binding: Pat,
    pub collection: Expr,
    pub body: Vec<ViewNode>,
    pub span: Span,
}

pub(crate) struct Argument {
    pub name: Option<Ident>,
    pub value: Expr,
}
impl Argument {
    pub fn start_offset(&self) -> usize {
        self.name.as_ref().map_or_else(
            || self.value.span().byte_range().start,
            |ident| ident.span().byte_range().start,
        )
    }
    pub fn end_offset(&self) -> usize {
        self.value.span().byte_range().end
    }
}
impl quote::ToTokens for Argument {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        quote::ToTokens::to_tokens(&self.value, tokens);
    }
}
pub(crate) struct NamedViews(pub Vec<NamedView>);

pub(crate) struct NamedView {
    pub preview: Option<proc_macro2::Span>,
    pub range: std::ops::Range<usize>,
    pub attributes: Vec<syn::Attribute>,
    pub visibility: syn::Visibility,
    pub name: Ident,
    pub properties: Vec<Property>,
    pub statements: Vec<syn::Stmt>,
    pub states: Vec<StateDecl>,
    pub tree: ViewTree,
}
pub(crate) struct Property {
    pub name: Ident,
    pub ty: syn::Type,
}

pub(crate) struct StateDecl {
    pub name: Ident,
    pub ty: Option<syn::Type>,
    pub initialize: Option<Expr>,
    pub environment: bool,
    pub responsive: bool,
    pub localized: bool,
    pub statement_index: usize,
}
