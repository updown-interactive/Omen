use omen_ui::{Color, Font, Style, View, ViewKind, ViewNode, Weight, view};
use std::{cell::Cell, rc::Rc};

fn text_contents(node: &ViewNode) -> Vec<String> {
    match &node.kind {
        ViewKind::Text(text) | ViewKind::Image(text) | ViewKind::Icon(text) => vec![text.clone()],
        ViewKind::TextField { label, .. }
        | ViewKind::SecureField { label, .. }
        | ViewKind::Checkbox { label, .. }
        | ViewKind::Toggle { label, .. } => vec![label.clone()],
        ViewKind::Reactive(_) => {
            text_contents(&omen_ui::Runtime::new(node.clone()).render().unwrap())
        }
        ViewKind::Progress(_) | ViewKind::Gap(_) | ViewKind::Divider => Vec::new(),
        ViewKind::Button { label, .. } => vec![label.clone()],
        ViewKind::Presentation { children, .. }
        | ViewKind::Layout(children)
        | ViewKind::Group(children)
        | ViewKind::List(children)
        | ViewKind::Card(children) => children.iter().flat_map(text_contents).collect(),
    }
}

#[test]
fn conditions_loops_and_lists_preserve_source_order() {
    let node = view! { Layout {
        Text("first")
        if false { Text("hidden") } else if true { Text("visible") } else { Text("hidden") }
        if false { Text("hidden") }
        for value in ["a", "b"] { Text(value) Text("detail") }
        List([("id", "row")]) { (_, value) in Layout { Text(value) } }
        List(Vec::<String>::new()) { value in Text(value) }
        Text("last")
    } }
    .into_node();
    assert_eq!(
        text_contents(&node),
        [
            "first", "visible", "a", "detail", "b", "detail", "row", "last"
        ]
    );
}

#[test]
fn modifiers_produce_typed_runtime_styles_and_later_values_win() {
    let node = view! { Text("a").font(.title).font(.largeTitle).weight(.bold)
    .color(.green).padding(16).background(.blue).cornerRadius(12) }
    .into_node();
    assert_eq!(
        node.style,
        Style {
            font: Some(Font::LargeTitle),
            weight: Some(Weight::Bold),
            color: Some(Color::Green),
            padding: Some(16.),
            background: Some(Color::Blue.into()),
            corner_radius: Some(12.),
            source_site: node.style.source_site,
            ..Style::default()
        }
    );
}

#[test]
fn callbacks_are_deferred_reusable_and_keep_owned_captures_alive() {
    let count = Rc::new(Cell::new(0));
    let captured = Rc::clone(&count);
    let node = view! { Button("Increment") { captured.set(captured.get() + 1); } }.into_node();
    assert_eq!(count.get(), 0);
    let ViewKind::Button { action, .. } = node.kind else {
        panic!("expected button")
    };
    action.invoke();
    action.clone().invoke();
    assert_eq!(count.get(), 2);
}

#[test]
fn list_is_a_distinct_eager_runtime_node_with_row_groups() {
    let node = view! { List(["row"]) { item in Text(item) Text("detail") } }.into_node();
    let ViewKind::List(rows) = node.kind else {
        panic!("expected list")
    };
    assert_eq!(rows.len(), 1);
    assert!(matches!(&rows[0].kind, ViewKind::Group(children) if children.len() == 2));
}

#[test]
fn inactive_branches_do_not_evaluate_expressions() {
    let calls = Cell::new(0);
    let _ = view! { if false { Text({ calls.set(1); "hidden" }) } };
    assert_eq!(calls.get(), 0);
}

#[test]
fn layout_modes_are_typed_and_default_to_vertical() {
    use omen_ui::LayoutMode;
    let default = view! { Layout { Text("a") } }.into_node();
    assert!(matches!(default.kind, ViewKind::Layout(_)));
    assert_eq!(default.style.mode.unwrap_or_default(), LayoutMode::Vertical);
    for (node, expected) in [
        (
            view! { Layout {}.mode(.horizontal) }.into_node(),
            LayoutMode::Horizontal,
        ),
        (
            view! { Layout {}.mode(.vertical) }.into_node(),
            LayoutMode::Vertical,
        ),
        (
            view! { Layout {}.mode(.stack) }.into_node(),
            LayoutMode::Stack,
        ),
        (
            view! { Layout {}.mode(.stack).mode(LayoutMode::Horizontal) }.into_node(),
            LayoutMode::Horizontal,
        ),
    ] {
        assert_eq!(node.style.mode, Some(expected));
    }
}

mod reusable {
    use omen_ui::view;
    view! {
        pub Header(title: String, subtitle: String) {
            let heading = format!("{title} — {subtitle}");
            Layout { Text(heading) }
        }
    }
    view! { pub HomeScreen() { Header(subtitle: "Stay productive", title: "My Tasks") } }
}

#[test]
fn reusable_views_compose_across_modules_and_convert_named_properties() {
    use reusable::HomeScreen;
    let node = view! { HomeScreen() }.into_node();
    assert_eq!(text_contents(&node), ["My Tasks — Stay productive"]);
}

view! {
    CallbackView(label: String, calls: Rc<Cell<usize>>) {
        let title = format!("Action: {label}");
        Button(title) { calls.set(calls.get() + 1); }
    }
}

#[test]
fn named_views_preserve_owned_callback_captures() {
    let calls = Rc::new(Cell::new(0));
    let node = view! { CallbackView(calls: Rc::clone(&calls), label: "Save") }.into_node();
    assert_eq!(text_contents(&node), ["Action: Save"]);
    assert_eq!(calls.get(), 0);
    let mut runtime = omen_ui::Runtime::new(node);
    let resolved = runtime.render().unwrap();
    let ViewKind::Group(children) = &resolved.kind else {
        panic!("expected named view boundary")
    };
    let ViewKind::Button { action, .. } = &children[0].kind else {
        panic!("expected button")
    };
    action.invoke();
    assert_eq!(calls.get(), 1);
}

#[test]
fn controls_and_typed_appearance_preserve_runtime_data() {
    use omen_ui::{Alignment, Background, BoolValue, Frame, TextValue};
    let text = TextValue::new("Ada".to_string());
    let flag = BoolValue::new(true);
    let root = view! { Card {
        TextField("Name").value(text.clone()).frame(240, 44).accessibilityLabel("Full name")
        SecureField("Password")
        Checkbox("Accept").checked(flag.clone())
        Toggle("Notify")
        Image("sample.png").frame(80, 80)
        Icon("check").color(.green)
        Progress(0.5)
        Divider()
    }.spacing(8).alignment(.topLeading).gradient(.blue, .purple, 90).cornerRadius(12) }
    .into_node();
    assert_eq!(root.style.alignment, Some(Alignment::TopLeading));
    assert!(matches!(
        root.style.background,
        Some(Background::Gradient(_))
    ));
    let ViewKind::Card(children) = root.kind else {
        panic!("expected card")
    };
    assert_eq!(children.len(), 8);
    assert_eq!(
        children[0].style.frame,
        Some(Frame {
            width: 240.,
            height: 44.
        })
    );
    assert_eq!(children[0].style.text_value.as_ref().unwrap().get(), "Ada");
    assert_eq!(
        children[0].style.accessibility.label.as_deref(),
        Some("Full name")
    );
    assert!(children[2].style.checked.as_ref().unwrap().get());
}

#[test]
fn progress_clamps_values_and_shared_secrets_are_redacted() {
    use omen_ui::{Progress, TextValue};
    for (value, expected) in [(-1., 0.), (2., 1.), (f64::NAN, 0.)] {
        assert!(
            matches!(Progress::new(value).into_node().kind, ViewKind::Progress(fraction) if fraction == expected)
        );
    }
    let secret = TextValue::new("private-password".to_string());
    let node = view! { SecureField("Password").value(secret.clone()) }.into_node();
    assert!(!format!("{node:?}").contains("private-password"));
    assert_eq!(secret.get(), "private-password");
}

#[test]
fn grouped_views_are_ordinary_runtime_views() {
    view! {
        @Design
        HomePreviewExample {
            Layout { PreviewHeaderExample() }
        }

        PreviewHeaderExample {
            Text("Grouped header")
        }
    }

    let mut runtime = omen_ui::Runtime::new(view! { HomePreviewExample() }.into_node());
    let rendered = runtime.render().unwrap();
    assert_eq!(text_contents(&rendered), ["Grouped header"]);
}
