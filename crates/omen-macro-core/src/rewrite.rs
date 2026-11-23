use crate::ast::*;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use syn::{
    Expr, Pat, Token, parse::Parser, punctuated::Punctuated, visit::Visit, visit_mut::VisitMut,
};

#[derive(Clone)]
struct State {
    ident: syn::Ident,
    direct: bool,
    environment: bool,
}
type States = HashMap<String, State>;
fn read_method(name: &str) -> bool {
    matches!(
        name,
        "clone"
            | "len"
            | "is_empty"
            | "is_some"
            | "is_none"
            | "as_ref"
            | "as_str"
            | "as_slice"
            | "iter"
            | "get"
            | "contains"
            | "contains_key"
            | "starts_with"
            | "ends_with"
            | "to_string"
            | "to_owned"
            | "chars"
            | "bytes"
            | "first"
            | "last"
            | "capacity"
            | "locale"
            | "text"
            | "format"
            | "entries"
            | "destination"
    )
}
fn bindings_in(pattern: &Pat) -> Vec<String> {
    struct Names(Vec<String>);
    impl<'ast> Visit<'ast> for Names {
        fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
            self.0.push(pattern.ident.to_string());
        }
    }
    let mut names = Names(Vec::new());
    names.visit_pat(pattern);
    names.0
}
fn marker(expr: &Expr) -> Option<syn::Ident> {
    if let Expr::Macro(mac) = expr
        && mac.mac.path.is_ident("__omen_binding")
    {
        syn::parse2(mac.mac.tokens.clone()).ok()
    } else {
        None
    }
}
fn projection(expr: &Expr) -> Option<(syn::Ident, Vec<syn::Member>)> {
    if let Some(name) = marker(expr) {
        return Some((name, Vec::new()));
    }
    if let Expr::Field(field) = expr {
        let (name, mut fields) = projection(&field.base)?;
        fields.push(field.member.clone());
        Some((name, fields))
    } else {
        None
    }
}
fn place_root(expression: &Expr) -> Option<&syn::Ident> {
    match expression {
        Expr::Path(path) => path.path.get_ident(),
        Expr::Field(field) => place_root(&field.base),
        Expr::Index(index) => place_root(&index.expr),
        Expr::Paren(paren) => place_root(&paren.expr),
        Expr::Group(group) => place_root(&group.expr),
        _ => None,
    }
}
fn chain_root(expression: &Expr) -> Option<&syn::Ident> {
    match expression {
        Expr::MethodCall(call) => chain_root(&call.receiver),
        Expr::Unary(unary) => chain_root(&unary.expr),
        Expr::Field(field) => chain_root(&field.base),
        Expr::Index(index) => chain_root(&index.expr),
        Expr::Paren(paren) => chain_root(&paren.expr),
        Expr::Group(group) => chain_root(&group.expr),
        _ => place_root(expression),
    }
}
fn unknown_chain(expression: &Expr) -> bool {
    if let Expr::MethodCall(call) = expression {
        !read_method(&call.method.to_string()) || unknown_chain(&call.receiver)
    } else {
        false
    }
}
fn unsupported(expression: &mut Expr, message: &str) {
    use syn::spanned::Spanned;
    *expression =
        syn::parse_quote_spanned!(expression.span()=> { ::core::compile_error!(#message) });
}
fn callback_reads(expression: &Expr, name: &syn::Ident) -> bool {
    struct Reads<'a> {
        name: &'a syn::Ident,
        inside: bool,
        found: bool,
    }
    impl<'ast> Visit<'ast> for Reads<'_> {
        fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
            if closure
                .inputs
                .iter()
                .flat_map(bindings_in)
                .any(|binding| *self.name == binding)
            {
                return;
            }
            let previous = self.inside;
            self.inside = true;
            self.visit_expr(&closure.body);
            self.inside = previous;
        }
        fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
            if self.inside && path.path.is_ident(self.name) {
                self.found = true;
            }
        }
    }
    let mut reads = Reads {
        name,
        inside: false,
        found: false,
    };
    reads.visit_expr(expression);
    reads.found
}
struct Rewrite {
    states: States,
    used: HashSet<String>,
    actions: bool,
    next_index: usize,
    batch_closures: bool,
}
impl Rewrite {
    fn new(states: &States) -> Self {
        Self {
            states: states.clone(),
            used: HashSet::new(),
            actions: false,
            next_index: 0,
            batch_closures: false,
        }
    }
    fn clone_bindings(&self) -> Vec<TokenStream> {
        let mut names = self.used.iter().collect::<Vec<_>>();
        names.sort();
        names
            .iter()
            .filter_map(|name| self.states.get(*name).map(|state| &state.ident))
            .map(|name| quote!(let #name = #name.clone();))
            .collect()
    }
    fn state_place(&mut self, expr: &Expr) -> Option<(syn::Ident, Expr, Vec<TokenStream>)> {
        match expr {
            Expr::Paren(paren) => self.state_place(&paren.expr),
            Expr::Group(group) => self.state_place(&group.expr),
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.get_ident()?;
                if self.states.contains_key(&name.to_string()) {
                    self.used.insert(name.to_string());
                    let value = syn::Ident::new("__omen_value", Span::mixed_site());
                    Some((name.clone(), syn::parse_quote!((*#value)), Vec::new()))
                } else {
                    None
                }
            }
            Expr::Field(field) => {
                let (name, base, setup) = self.state_place(&field.base)?;
                let mut place = field.clone();
                place.base = Box::new(base);
                Some((name, Expr::Field(place), setup))
            }
            Expr::Index(index) => {
                let (name, base, mut setup) = self.state_place(&index.expr)?;
                let mut place = index.clone();
                place.expr = Box::new(base);
                self.visit_expr_mut(&mut place.index);
                let index = &place.index;
                let temporary = syn::Ident::new(
                    &format!("__omen_index_{}", self.next_index),
                    Span::mixed_site(),
                );
                self.next_index += 1;
                setup.push(quote!(let #temporary=#index;));
                place.index = Box::new(syn::parse_quote!(#temporary));
                Some((name, Expr::Index(place), setup))
            }
            _ => None,
        }
    }
}
impl VisitMut for Rewrite {
    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        let original = self.states.clone();
        for statement in &mut block.stmts {
            self.visit_stmt_mut(statement);
            if let syn::Stmt::Local(local) = statement {
                for name in bindings_in(&local.pat) {
                    self.states.remove(&name);
                }
            }
        }
        self.states = original;
    }
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // Async methods may borrow self until their future completes. Snapshot only
        // for an explicit awaited call; never keep a RefCell borrow across await.
        if let Expr::Await(awaited) = expr
            && let Expr::MethodCall(call) = awaited.base.as_mut()
            && let Expr::Path(path) = call.receiver.as_ref()
            && let Some(name) = path.path.get_ident()
            && self
                .states
                .get(&name.to_string())
                .is_some_and(|state| state.environment)
        {
            self.used.insert(name.to_string());
            for argument in &mut call.args {
                self.visit_expr_mut(argument);
            }
            let method = &call.method;
            let args = &call.args;
            let turbofish = &call.turbofish;
            *expr = syn::parse_quote!({ #[allow(unused_mut)] let mut __omen_env = #name.get(); __omen_env.#method #turbofish (#args).await });
            return;
        }
        if let Expr::Reference(reference) = expr
            && reference.mutability.is_some()
            && chain_root(&reference.expr)
                .is_some_and(|name| self.states.contains_key(&name.to_string()))
        {
            unsupported(
                expr,
                "cannot borrow reactive state mutably; use state.update(|value| { ... }) instead",
            );
            return;
        }
        if let Expr::MethodCall(call) = expr
            && chain_root(&call.receiver)
                .is_some_and(|name| self.states.contains_key(&name.to_string()))
            && (matches!(
                call.method.to_string().as_str(),
                "iter_mut"
                    | "get_mut"
                    | "as_mut"
                    | "first_mut"
                    | "last_mut"
                    | "values_mut"
                    | "entry"
                    | "split_at_mut"
            ) || (self.actions && unknown_chain(&call.receiver))
                || (self.actions
                    && place_root(&call.receiver).is_none()
                    && !matches!(call.receiver.as_ref(), Expr::MethodCall(_))
                    && !read_method(&call.method.to_string())))
        {
            unsupported(
                expr,
                "reactive mutation through a borrowed method chain is unsupported; use state.update(|value| { ... }) instead",
            );
            return;
        }
        if let Some((name, fields)) = projection(expr) {
            if self.states.contains_key(&name.to_string()) {
                self.used.insert(name.to_string());
            }
            *expr = if !fields.is_empty()
                && self
                    .states
                    .get(&name.to_string())
                    .is_some_and(|state| state.environment)
            {
                syn::parse_quote!({
                    use ::omen_ui::BindEnvironmentField as _;
                    ::omen_ui::EnvironmentProjection {
                        model: #name.clone(),
                        read: |model| &model #(.#fields)*,
                        write: |model| &mut model #(.#fields)*,
                    }.binding()
                })
            } else if fields.is_empty() {
                syn::parse_quote!(#name.clone())
            } else {
                let read = syn::Ident::new("__omen_read", Span::mixed_site());
                let write = syn::Ident::new("__omen_write", Span::mixed_site());
                syn::parse_quote!({
                    let #read = #name.clone(); let #write = #name.clone();
                    ::omen_ui::Binding::new(move || #read.get() #(.#fields)* .clone(), move |__omen_next| #write.update(|__omen_model| { __omen_model #(.#fields)* = __omen_next; }))
                })
            };
            return;
        }
        // Borrow injected models; clone only the requested field or return value.
        if let Expr::MethodCall(call) = expr {
            if let Expr::Path(path) = call.receiver.as_ref()
                && let Some(name) = path.path.get_ident()
                && self
                    .states
                    .get(&name.to_string())
                    .is_some_and(|state| state.environment)
                && call.method != "update"
                && call.method != "get"
                && call.method != "read"
            {
                self.used.insert(name.to_string());
                for argument in &mut call.args {
                    self.visit_expr_mut(argument);
                }
                let method = &call.method;
                let args = &call.args;
                let turbofish = &call.turbofish;
                let access = if self.actions && !read_method(&method.to_string()) {
                    syn::Ident::new("update", Span::mixed_site())
                } else {
                    syn::Ident::new("read", Span::mixed_site())
                };
                *expr = syn::parse_quote!(#name.#access(|__omen_env| __omen_env.#method #turbofish (#args)));
                return;
            }
            if let Expr::Field(field) = call.receiver.as_ref()
                && let Expr::Path(path) = field.base.as_ref()
                && let Some(name) = path.path.get_ident()
                && self
                    .states
                    .get(&name.to_string())
                    .is_some_and(|state| state.environment)
            {
                self.used.insert(name.to_string());
                for argument in &mut call.args {
                    self.visit_expr_mut(argument);
                }
                let member = &field.member;
                let method = &call.method;
                let args = &call.args;
                let turbofish = &call.turbofish;
                // Preserve explicit Value methods and existing ordinary-field methods.
                let access = if read_method(&method.to_string())
                    || matches!(method.to_string().as_str(), "set")
                {
                    syn::Ident::new("read", Span::mixed_site())
                } else {
                    syn::Ident::new("update", Span::mixed_site())
                };
                *expr = syn::parse_quote!(#name.#access(|__omen_env| __omen_env.#member.#method #turbofish (#args)));
                return;
            }
        }
        if let Expr::Field(field) = expr
            && let Expr::Path(path) = field.base.as_ref()
            && let Some(name) = path.path.get_ident()
            && self
                .states
                .get(&name.to_string())
                .is_some_and(|state| state.environment)
        {
            self.used.insert(name.to_string());
            let member = &field.member;
            *expr = syn::parse_quote!(#name.read(|__omen_env| {
                use ::omen_ui::ReadEnvironmentField as _;
                ::omen_ui::EnvironmentField(&__omen_env.#member).snapshot()
            }));
            return;
        }
        match expr {
            Expr::MethodCall(call) if call.method == "update" => {
                if let Expr::Path(path) = call.receiver.as_ref()
                    && let Some(name) = path.path.get_ident()
                    && self.states.contains_key(&name.to_string())
                {
                    self.used.insert(name.to_string());
                    for argument in &mut call.args {
                        self.visit_expr_mut(argument);
                    }
                    return;
                }
            }
            Expr::Assign(assign) => {
                if chain_root(&assign.left)
                    .is_some_and(|name| self.states.contains_key(&name.to_string()))
                    && place_root(&assign.left).is_none()
                {
                    unsupported(
                        expr,
                        "unsupported reactive assignment target; use state.update(|value| { ... }) instead",
                    );
                    return;
                }
                if let Some((name, place, setup)) = self.state_place(&assign.left) {
                    self.visit_expr_mut(&mut assign.right);
                    let value = &assign.right;
                    let value = if matches!(value.as_ref(),Expr::Lit(literal) if matches!(literal.lit,syn::Lit::Str(_)))
                    {
                        quote!(::core::convert::Into::into(#value))
                    } else {
                        quote!(#value)
                    };
                    let parameter = syn::Ident::new("__omen_value", Span::mixed_site());
                    if self
                        .states
                        .get(&name.to_string())
                        .is_some_and(|state| state.environment)
                        && matches!(assign.left.as_ref(), Expr::Field(_))
                    {
                        *expr = syn::parse_quote!({
                            let __omen_rhs = #value;
                            #(#setup)*
                            use ::omen_ui::BindEnvironmentField as _;
                            ::omen_ui::EnvironmentProjection {
                                model: #name.clone(),
                                read: |#parameter| &#place,
                                write: |#parameter| &mut #place,
                            }.binding().set(__omen_rhs)
                        });
                    } else {
                        *expr = syn::parse_quote!({ let __omen_rhs = #value; #(#setup)* #name.update(|#parameter| { #place = __omen_rhs; }) });
                    }
                    return;
                }
            }
            Expr::Binary(binary)
                if matches!(
                    binary.op,
                    syn::BinOp::AddAssign(_)
                        | syn::BinOp::SubAssign(_)
                        | syn::BinOp::MulAssign(_)
                        | syn::BinOp::DivAssign(_)
                        | syn::BinOp::RemAssign(_)
                        | syn::BinOp::BitXorAssign(_)
                        | syn::BinOp::BitAndAssign(_)
                        | syn::BinOp::BitOrAssign(_)
                        | syn::BinOp::ShlAssign(_)
                        | syn::BinOp::ShrAssign(_)
                ) =>
            {
                if chain_root(&binary.left)
                    .is_some_and(|name| self.states.contains_key(&name.to_string()))
                    && place_root(&binary.left).is_none()
                {
                    unsupported(
                        expr,
                        "unsupported reactive assignment target; use state.update(|value| { ... }) instead",
                    );
                    return;
                }
                if let Some((name, place, setup)) = self.state_place(&binary.left) {
                    self.visit_expr_mut(&mut binary.right);
                    let value = &binary.right;
                    let op = &binary.op;
                    let parameter = syn::Ident::new("__omen_value", Span::mixed_site());
                    *expr = syn::parse_quote!({ let __omen_rhs = #value; #(#setup)* #name.update(|#parameter| { #place #op __omen_rhs; }) });
                    return;
                }
            }
            Expr::MethodCall(call)
                if (self.actions
                    && place_root(&call.receiver).is_some_and(|name| {
                        self.states
                            .get(&name.to_string())
                            .is_some_and(|state| state.direct)
                    })
                    && !read_method(&call.method.to_string()))
                    || matches!(
                        call.method.to_string().as_str(),
                        "toggle"
                            | "push"
                            | "pop"
                            | "insert"
                            | "remove"
                            | "clear"
                            | "retain"
                            | "sort"
                            | "sort_by"
                            | "reverse"
                            | "extend"
                            | "truncate"
                    ) =>
            {
                if let Some((name, place, setup)) = self.state_place(&call.receiver) {
                    if call
                        .args
                        .iter()
                        .any(|argument| callback_reads(argument, &name))
                    {
                        unsupported(
                            expr,
                            "a reactive mutation callback cannot capture the same state; read needed values before the call, or use one state.update closure",
                        );
                        return;
                    }
                    for argument in &mut call.args {
                        self.visit_expr_mut(argument);
                    }
                    let method = &call.method;
                    let turbofish = &call.turbofish;
                    let arguments = call.args.iter().collect::<Vec<_>>();
                    let parameter = syn::Ident::new("__omen_value", Span::mixed_site());
                    let values = (0..arguments.len())
                        .map(|index| {
                            syn::Ident::new(&format!("__omen_argument_{index}"), Span::mixed_site())
                        })
                        .collect::<Vec<_>>();
                    *expr = syn::parse_quote!({
                        #(#setup)*
                        let (#(#values,)*) = (#(#arguments,)*);
                        #name.update(|#parameter| #place.#method #turbofish (#(#values),*))
                    });
                    return;
                }
            }
            Expr::ForLoop(repeated) => {
                self.visit_expr_mut(&mut repeated.expr);
                let original = self.states.clone();
                for name in bindings_in(&repeated.pat) {
                    self.states.remove(&name);
                }
                self.visit_block_mut(&mut repeated.body);
                self.states = original;
                return;
            }
            Expr::If(conditional) => {
                self.visit_expr_mut(&mut conditional.cond);
                let original = self.states.clone();
                if let Expr::Let(binding) = conditional.cond.as_ref() {
                    for name in bindings_in(&binding.pat) {
                        self.states.remove(&name);
                    }
                }
                self.visit_block_mut(&mut conditional.then_branch);
                self.states = original;
                if let Some((_, otherwise)) = &mut conditional.else_branch {
                    self.visit_expr_mut(otherwise);
                }
                return;
            }
            Expr::Match(matched) => {
                self.visit_expr_mut(&mut matched.expr);
                let original = self.states.clone();
                for arm in &mut matched.arms {
                    self.states = original.clone();
                    for name in bindings_in(&arm.pat) {
                        self.states.remove(&name);
                    }
                    if let Some((_, guard)) = &mut arm.guard {
                        self.visit_expr_mut(guard);
                    }
                    self.visit_expr_mut(&mut arm.body);
                }
                self.states = original;
                return;
            }
            Expr::Closure(closure) => {
                let mut nested = Self::new(&self.states);
                nested.actions = self.actions;
                for input in &closure.inputs {
                    for name in bindings_in(input) {
                        nested.states.remove(&name);
                    }
                }
                nested.visit_expr_mut(&mut closure.body);
                if self.batch_closures
                    && closure.asyncness.is_none()
                    && !matches!(closure.body.as_ref(), Expr::Async(_))
                {
                    let body = &closure.body;
                    *closure.body = syn::parse_quote!(::omen_ui::batch_updates(|| #body));
                }
                let clones = nested.clone_bindings();
                self.used.extend(nested.used);
                if !clones.is_empty() {
                    closure.capture = Some(Default::default());
                    let closure = closure.clone();
                    *expr = syn::parse_quote!({ #(#clones)* #closure });
                }
                return;
            }
            Expr::Async(asynchronous) => {
                let mut nested = Self::new(&self.states);
                nested.actions = self.actions;
                nested.visit_block_mut(&mut asynchronous.block);
                let clones = nested.clone_bindings();
                self.used.extend(nested.used);
                if !clones.is_empty() {
                    asynchronous.capture = Some(Default::default());
                    let asynchronous = asynchronous.clone();
                    *expr = syn::parse_quote!({ #(#clones)* #asynchronous });
                }
                return;
            }
            Expr::Path(path) if path.path.segments.len() == 1 => {
                if let Some(name) = path.path.get_ident()
                    && self.states.contains_key(&name.to_string())
                {
                    self.used.insert(name.to_string());
                    *expr = syn::parse_quote!(#name.get());
                    return;
                }
            }
            Expr::Macro(mac) => {
                if let Ok(mut arguments) =
                    Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.mac.tokens.clone())
                {
                    let mut captures = Vec::new();
                    if matches!(
                        mac.mac
                            .path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .as_deref(),
                        Some(
                            "format"
                                | "format_args"
                                | "println"
                                | "print"
                                | "eprintln"
                                | "eprint"
                                | "panic"
                        )
                    ) && let Some(Expr::Lit(literal)) = arguments.first()
                        && let syn::Lit::Str(text) = &literal.lit
                    {
                        for (name, ident) in &self.states {
                            if text.value().contains(&format!("{{{name}"))
                                && !arguments.iter().any(|arg| matches!(arg, Expr::Assign(assign) if matches!(assign.left.as_ref(), Expr::Path(path) if path.path.is_ident(name))))
                            {
                                self.used.insert(name.clone());
                                captures.push(ident.ident.clone());
                            }
                        }
                        captures.sort_by_key(ToString::to_string);
                    }
                    for argument in &mut arguments {
                        if let Expr::Assign(named) = argument {
                            self.visit_expr_mut(&mut named.right);
                        } else {
                            self.visit_expr_mut(argument);
                        }
                    }
                    for name in captures {
                        arguments.push(syn::parse_quote!(#name = #name.get()));
                    }
                    mac.mac.tokens = arguments.into_token_stream();
                }
                return;
            }
            _ => {}
        }
        syn::visit_mut::visit_expr_mut(self, expr);
    }
}
fn action(block: &syn::Block, asynchronous: bool, states: &States) -> Expr {
    if asynchronous {
        let mut expression: Expr = syn::parse_quote!(move || async move #block);
        let mut rewrite = Rewrite::new(states);
        rewrite.actions = true;
        rewrite.visit_expr_mut(&mut expression);
        return expression;
    }
    let mut block = block.clone();
    if let Some(syn::Stmt::Expr(Expr::MethodCall(call), semicolon)) = block.stmts.last_mut()
        && matches!(
            call.method.to_string().as_str(),
            "push" | "push_view" | "pop" | "replace"
        )
    {
        *semicolon = Some(Default::default());
    }
    let mut rewrite = Rewrite::new(states);
    rewrite.actions = true;
    rewrite.visit_block_mut(&mut block);
    let clones = rewrite.clone_bindings();
    // Insert batching after rewriting so the compiler-generated inner closure
    // borrows ordinary captures rather than moving them out of a repeatable Fn.
    syn::parse_quote!({ #(#clones)* move || ::omen_ui::batch_updates(|| #block) })
}

fn nodes(items: &mut [ViewNode], states: &States) {
    for node in items {
        match node {
            ViewNode::Element(element) => {
                for argument in &mut element.arguments {
                    Rewrite::new(states).visit_expr_mut(&mut argument.value);
                }
                for modifier in &mut element.modifiers {
                    for argument in &mut modifier.arguments {
                        match argument {
                            ModifierArg::Views(tree) => {
                                nodes(&mut tree.0, states);
                                let content = crate::codegen::generate(tree)
                                    .expect("validated presentation content");
                                struct Used(HashSet<String>);
                                impl<'a> Visit<'a> for Used {
                                    fn visit_expr_path(&mut self, path: &'a syn::ExprPath) {
                                        if let Some(name) = path.path.get_ident() {
                                            self.0.insert(name.to_string());
                                        }
                                        syn::visit::visit_expr_path(self, path);
                                    }
                                }
                                let expression: Expr = syn::parse2(content.clone())
                                    .expect("generated view expression");
                                let mut used = Used(HashSet::new());
                                used.visit_expr(&expression);
                                let mut names = states
                                    .iter()
                                    .filter(|(name, _)| used.0.contains(*name))
                                    .map(|(_, state)| &state.ident)
                                    .collect::<Vec<_>>();
                                names.sort_by_key(ToString::to_string);
                                *argument = ModifierArg::Rust(
                                    syn::parse_quote!({ #(let #names = #names.clone();)* move || #content }),
                                );
                            }
                            ModifierArg::Rust(expression) => {
                                let mut rewrite = Rewrite::new(states);
                                rewrite.actions = matches!(
                                    modifier.name.to_string().as_str(),
                                    "onTap"
                                        | "onContextMenu"
                                        | "shortcut"
                                        | "onTapAsync"
                                        | "onChange"
                                        | "onLongPress"
                                        | "onSwipe"
                                        | "onAppear"
                                        | "onDisappear"
                                        | "onWindowClose"
                                        | "onWindowBlur"
                                        | "effect"
                                        | "task"
                                        | "onError"
                                );
                                rewrite.batch_closures = rewrite.actions;
                                rewrite.visit_expr_mut(expression)
                            }
                            ModifierArg::Action {
                                block,
                                asynchronous,
                            } => {
                                *argument = ModifierArg::Rust(action(block, *asynchronous, states))
                            }
                            ModifierArg::Symbol(_) | ModifierArg::Adaptive(_) => {}
                        }
                    }
                }
                let asynchronous = matches!(element.content, Content::AsyncAction(_));
                match &mut element.content {
                    Content::Action(block) | Content::AsyncAction(block) => {
                        let expression = action(block, asynchronous, states);
                        // Store the factory expression for code generation in a synthetic argument.
                        element.arguments.push(Argument {
                            name: Some(syn::Ident::new("__omen_action", Span::mixed_site())),
                            value: expression,
                        });
                    }
                    Content::Children(children) => nodes(children, states),
                    Content::List { binding, body } => {
                        let mut nested = states.clone();
                        for name in bindings_in(binding) {
                            nested.remove(&name);
                        }
                        nodes(body, &nested);
                    }
                    Content::None => {}
                }
            }
            ViewNode::Conditional(conditional) => {
                Rewrite::new(states).visit_expr_mut(&mut conditional.condition);
                let mut then_states = states.clone();
                if let Expr::Let(binding) = &conditional.condition {
                    for name in bindings_in(&binding.pat) {
                        then_states.remove(&name);
                    }
                }
                nodes(&mut conditional.then_branch, &then_states);
                nodes(&mut conditional.else_branch, states);
            }
            ViewNode::Loop(repeated) => {
                Rewrite::new(states).visit_expr_mut(&mut repeated.collection);
                let mut nested = states.clone();
                for name in bindings_in(&repeated.binding) {
                    nested.remove(&name);
                }
                nodes(&mut repeated.body, &nested);
            }
        }
    }
}
pub(crate) fn anonymous(tree: &mut ViewTree) {
    nodes(&mut tree.0, &States::new());
}
pub(crate) fn named(view: &mut NamedView) -> syn::Result<()> {
    let mut states = States::new();
    let mut all_names = HashSet::new();
    for state in &view.states {
        if !all_names.insert(state.name.to_string())
            || view
                .properties
                .iter()
                .any(|property| property.name == state.name)
        {
            return Err(syn::Error::new_spanned(
                &state.name,
                "duplicate state/environment/property name",
            ));
        }
    }
    for index in 0..=view.statements.len() {
        for state in view
            .states
            .iter_mut()
            .filter(|state| state.statement_index == index)
        {
            if let Some(initialize) = &mut state.initialize {
                Rewrite::new(&states).visit_expr_mut(initialize);
            }
            if state.responsive {
                continue;
            }
            states.insert(
                state.name.to_string(),
                State {
                    ident: state.name.clone(),
                    direct: !state.environment && !state.localized,
                    environment: state.environment,
                },
            );
        }
        if let Some(statement) = view.statements.get_mut(index) {
            Rewrite::new(&states).visit_stmt_mut(statement);
            if let syn::Stmt::Local(local) = statement {
                for name in bindings_in(&local.pat) {
                    states.remove(&name);
                }
            }
        }
    }
    nodes(&mut view.tree.0, &states);
    Ok(())
}
