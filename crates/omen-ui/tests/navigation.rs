use omen_ui::{view, *};
fn children(node: &ViewNode) -> &[ViewNode] {
    match &node.kind {
        ViewKind::Group(c)
        | ViewKind::Layout(c)
        | ViewKind::List(c)
        | ViewKind::Card(c)
        | ViewKind::Presentation { children: c, .. } => c,
        _ => &[],
    }
}
fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind,ViewKind::Text(text)|ViewKind::Button{label:text,..} if text==label) {
        Some(node)
    } else {
        children(node).iter().find_map(|node| find(node, label))
    }
}
fn click(node: &ViewNode, label: &str) {
    fn button<'a>(node: &'a ViewNode, label: &str) -> Option<&'a Action> {
        if let ViewKind::Button {
            label: text,
            action,
        } = &node.kind
            && text == label
        {
            return Some(action);
        }
        children(node).iter().find_map(|node| button(node, label))
    }
    button(node, label)
        .unwrap_or_else(|| panic!("missing button {label}"))
        .invoke();
}
view! { Counter(label:String) {
    @State let count=0;
    Layout {Text(format!("{label}:{count}")) Button(label.clone()){count+=1;}}
} }
#[test]
fn navigation_retains_covered_pages_and_disposes_popped_visits() {
    let nav = Navigator::new();
    let root=view! { NavigationStack(nav.clone(), |route:Route| view!{Counter(label:route.name)}) {Counter(label:"home").navigationTitle("Home")} }.into_node();
    let mut runtime = Runtime::new(root);
    let root = runtime.render().unwrap();
    click(&root, "home");
    nav.push("detail");
    let root = runtime.render().unwrap();
    assert!(find(&root, "home:1").is_none());
    click(&root, "detail");
    nav.push("nested");
    runtime.render().unwrap();
    nav.pop();
    let root = runtime.render().unwrap();
    assert!(find(&root, "detail:1").is_some());
    click(&root, "Back");
    let root = runtime.render().unwrap();
    assert!(find(&root, "home:1").is_some());
    nav.push("detail");
    let root = runtime.render().unwrap();
    assert!(find(&root, "detail:0").is_some());
}

view! {
    ReleasePage(hide_bar: bool) {
        Layout { Text("Release notes") }
            .navigationTitle("Published")
            .showBackButton(false)
            .appBarVisible(!hide_bar)
            .centerTitle(true)
            .appBarBackground("#0D0D0F")
            .appBarColor("#EAEAED")
    }
}

#[test]
fn navigation_app_bar_reads_options_from_the_body_of_a_named_page() {
    let nav = Navigator::new();
    let root =
        NavigationStack::new(nav.clone(), |_| Text::new("Fallback"))
            .children([Text::new("Home").navigation_title("Overview").into_node()]);
    let mut runtime = Runtime::new(root.into_node());
    assert!(find(&runtime.render().unwrap(), "Overview").is_some());

    nav.push_view(ReleasePage { hide_bar: false });
    let detail = runtime.render().unwrap();
    assert!(find(&detail, "Published").is_some());
    assert!(find(&detail, "Back").is_none());

    nav.push_view(ReleasePage { hide_bar: true });
    let full_screen = runtime.render().unwrap();
    assert!(find(&full_screen, "Release notes").is_some());
    assert!(find(&full_screen, "Published").is_none());
}
#[test]
fn tab_selection_preserves_each_tabs_state() {
    let selection = Value::new("first".to_string());
    let root = view! { TabView(selection.clone()) {
        Tab("first","First"){Counter(label:"first")}
        Tab("second","Second"){Counter(label:"second")}
    }}
    .into_node();
    let mut runtime = Runtime::new(root);
    let root = runtime.render().unwrap();
    click(&root, "first");
    click(&root, "Second");
    let root = runtime.render().unwrap();
    assert!(find(&root, "second:0").is_some());
    click(&root, "second");
    click(&root, "First");
    let root = runtime.render().unwrap();
    assert!(find(&root, "first:1").is_some());
    assert_eq!(selection.get(), "first");
}
fn presented(node: &ViewNode) -> Option<&Action> {
    if let ViewKind::Presentation { dismiss, .. } = &node.kind {
        Some(dismiss)
    } else {
        children(node).iter().find_map(presented)
    }
}
#[test]
fn all_presentation_kinds_are_bound_and_preserve_the_base_view() {
    for kind in [
        PresentationKind::Sheet,
        PresentationKind::Dialog,
        PresentationKind::Popover,
        PresentationKind::ContextMenu,
        PresentationKind::Modal,
    ] {
        let visible = Value::new(false);
        let base = view! {Counter(label:"base")};
        let root = match kind {
            PresentationKind::Sheet => base.sheet(visible.clone(), || view! {Text("Presented")}),
            PresentationKind::Dialog => base.dialog(visible.clone(), || view! {Text("Presented")}),
            PresentationKind::Popover => {
                base.popover(visible.clone(), || view! {Text("Presented")})
            }
            PresentationKind::ContextMenu => {
                base.context_menu(visible.clone(), || view! {Text("Presented")})
            }
            PresentationKind::Modal => base.modal(visible.clone(), || view! {Text("Presented")}),
        }
        .into_node();
        let mut runtime = Runtime::new(root);
        let root = runtime.render().unwrap();
        click(&root, "base");
        visible.set(true);
        let root = runtime.render().unwrap();
        assert!(find(&root, "Presented").is_some());
        assert!(find(&root, "base:1").is_some());
        presented(&root).unwrap().invoke();
        assert!(!visible.get());
        let root = runtime.render().unwrap();
        assert!(find(&root, "Presented").is_none());
        assert!(find(&root, "base:1").is_some());
    }
}
#[test]
fn duplicate_tabs_return_a_reconciliation_error() {
    let root=view!{TabView(Value::new("same".to_string())){Tab("same","One"){Text("1")} Tab("same","Two"){Text("2")}}}.into_node();
    assert!(matches!(
        Runtime::new(root).render(),
        Err(RuntimeError::DuplicateKey(_))
    ));
}
#[test]
fn navigator_replaces_routes_with_new_visit_identity_and_keeps_parameters() {
    let nav = Navigator::new();
    assert!(nav.pop().is_none());
    nav.push(Route::new("detail").parameter("id", 42));
    let first = nav.entries()[0].clone();
    assert_eq!(first.route.parameters["id"], "42");
    nav.replace("other");
    assert_ne!(nav.entries()[0].id, first.id);
    assert_eq!(nav.entries().len(), 1);
    nav.reset();
    assert!(nav.entries().is_empty());
}

#[test]
fn dismissed_presentation_content_starts_a_new_lifetime() {
    let visible = Value::new(true);
    let root = view! {Text("Base")}
        .modal(visible.clone(), || view! {Counter(label:"modal")})
        .into_node();
    let mut runtime = Runtime::new(root);
    let root = runtime.render().unwrap();
    click(&root, "modal");
    let root = runtime.render().unwrap();
    assert!(find(&root, "modal:1").is_some());
    presented(&root).unwrap().invoke();
    runtime.render().unwrap();
    visible.set(true);
    let root = runtime.render().unwrap();
    assert!(find(&root, "modal:0").is_some());
}

#[test]
fn conditional_tab_removal_disposes_state_and_rejects_invalid_content() {
    let show = Value::new(true);
    let selected = Value::new("extra".to_string());
    let visible = show.clone();
    let selection = selected.clone();
    let root = ViewNode::new(ViewKind::Reactive(ReactiveView::new(
        "DynamicTabs",
        move |_| {
            Ok(view! {
                TabView(selection.clone()) {
                    Tab("main","Main"){Counter(label:"main")}
                    if visible.get(){Tab("extra","Extra"){Counter(label:"extra")}}
                }
            }
            .into_node())
        },
    )));
    let mut runtime = Runtime::new(root);
    click(&runtime.render().unwrap(), "extra");
    show.set(false);
    runtime.render().unwrap();
    show.set(true);
    assert!(find(&runtime.render().unwrap(), "extra:0").is_some());
    let invalid = view! {TabView(Value::new("main".to_string())){Text("invalid")}}.into_node();
    assert!(matches!(
        Runtime::new(invalid).render(),
        Err(RuntimeError::InvalidTabContent)
    ));
}
