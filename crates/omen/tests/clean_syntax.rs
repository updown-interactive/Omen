use omen::prelude::*;

view! {
    Detail(title: String) {
        @State count = 0
        Layout {
            Text(title.clone())
            Text("Detail count: {count}")
            Button("Increment detail") { count += 1 }
        }.navigationTitle(title)
    }
}
view! {
    Home {
        @State navigator = Navigator()
        @State dark = false
        @State sheet = false
        @State name = "initial"
        @Environment viewport
        @Localize strings {
            en { welcome: "Welcome" }
            ar { welcome: "مرحبا" }
        }
        NavigationStack($navigator) {
            Layout {
                Text(strings.welcome)
                Text(name)
                Text(format!("{}", viewport.width))
                Button("Theme") { dark.toggle() }
                Button("Rename") { name = "updated" }
                Button("Arabic") { strings.locale = "ar" }
                Button("Detail") { navigator.push(Detail(title: "Detail")) }
                Button("Sheet") { sheet = true }
            }.spacing(.lg).padding(.md).horizontal(.medium)
        }
        .sheet($sheet) { Text("Sheet content") Text(name) }
        .theme(if dark { .dark } else { .light })
        .locale(strings.locale)
        .offset(x: if dark { 64.0 } else { 0.0 })
        .animation(.spring(800.ms))
    }
}
fn find<'a>(node: &'a ViewNode, text: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind, ViewKind::Text(label)|ViewKind::Button{label,..} if label==text) {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Layout(children)
        | ViewKind::Group(children)
        | ViewKind::Card(children)
        | ViewKind::Presentation { children, .. } => {
            children.iter().find_map(|child| find(child, text))
        }
        _ => None,
    }
}
fn click(root: &ViewNode, label: &str) {
    let ViewKind::Button { action, .. } = &find(root, label).unwrap().kind else {
        panic!("missing button")
    };
    action.invoke();
}
#[test]
fn clean_syntax_preserves_state_navigation_and_localization() {
    let mut runtime = Runtime::new(view! {Home()}.into_node());
    let root = runtime.render().unwrap();
    assert!(find(&root, "Welcome").is_some());
    click(&root, "Rename");
    let root = runtime.render().unwrap();
    assert!(find(&root, "updated").is_some());
    click(&root, "Theme");
    click(&root, "Arabic");
    let root = runtime.render().unwrap();
    assert!(find(&root, "مرحبا").is_some());
    click(&root, "Sheet");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Sheet content").is_some());
    fn dismiss(node: &ViewNode) -> bool {
        if let ViewKind::Presentation { dismiss, .. } = &node.kind {
            dismiss.invoke();
            return true;
        }
        match &node.kind {
            ViewKind::Layout(children) | ViewKind::Group(children) => children.iter().any(dismiss),
            _ => false,
        }
    }
    assert!(dismiss(&root));
    let root = runtime.render().unwrap();
    assert!(find(&root, "Sheet content").is_none());
    click(&root, "Detail");
    let root = runtime.render().unwrap();
    click(&root, "Increment detail");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Detail count: 1").is_some());
    click(&root, "Back");
    let root = runtime.render().unwrap();
    assert!(find(&root, "updated").is_some());
    assert!(find(&root, "مرحبا").is_some());
}
#[test]
fn anonymous_root_supports_state_without_let_or_semicolons() {
    let mut runtime = Runtime::new(
        view! {
            @State count = 0
            Layout { Text("Count: {count}") Button("Increment") { count += 1 } }
        }
        .into_node(),
    );
    let root = runtime.render().unwrap();
    click(&root, "Increment");
    assert!(find(&runtime.render().unwrap(), "Count: 1").is_some());
}

#[test]
fn presentation_blocks_capture_live_state_and_actions() {
    let mut runtime = Runtime::new(
        view! {
            @State visible = true
            @State count = 0
            Text("Base").sheet($visible) {
                Text("Sheet count: {count}")
                Button("Sheet increment") { count += 1 }
            }
        }
        .into_node(),
    );
    let root = runtime.render().unwrap();
    click(&root, "Sheet increment");
    assert!(find(&runtime.render().unwrap(), "Sheet count: 1").is_some());
}
#[test]
fn typed_visits_retain_prior_screen_state_and_dispose_popped_state() {
    let navigator = Navigator::new();
    let controller = Value::new(navigator.clone());
    let mut runtime = Runtime::new(view! {NavigationStack(controller) {Text("Root")}}.into_node());
    runtime.render().unwrap();
    navigator.push_view(Detail {
        title: "First".into(),
    });
    let root = runtime.render().unwrap();
    click(&root, "Increment detail");
    runtime.render().unwrap();
    navigator.push_view(Detail {
        title: "Second".into(),
    });
    assert!(find(&runtime.render().unwrap(), "Detail count: 0").is_some());
    navigator.pop();
    assert!(find(&runtime.render().unwrap(), "Detail count: 1").is_some());
    navigator.pop();
    runtime.render().unwrap();
    navigator.push_view(Detail {
        title: "First".into(),
    });
    assert!(find(&runtime.render().unwrap(), "Detail count: 0").is_some());
}

#[test]
fn state_let_strings_and_viewport_fields_interpolate_and_update() {
    let mut runtime = Runtime::new(
        view! {
            @State let tab = "overview"
            @Environment viewport
            Layout {
                Text("Tab: {tab}")
                Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}")
                    .font(.caption)
                    .color(.secondary)
                Button("Change tab") { tab = "Overview" }
            }
        }
        .into_node(),
    );
    runtime.set_viewport(Viewport {
        width: 640.0,
        height: 480.0,
        ..Default::default()
    });
    let root = runtime.render().unwrap();
    assert!(find(&root, "Tab: overview").is_some());
    assert!(find(&root, "Viewport: Medium · 640 × 480").is_some());
    click(&root, "Change tab");
    runtime.set_viewport(Viewport {
        width: 1200.0,
        height: 800.0,
        ..Default::default()
    });
    let root = runtime.render().unwrap();
    assert!(find(&root, "Tab: Overview").is_some());
    assert!(find(&root, "Viewport: Large · 1200 × 800").is_some());
}
#[test]
fn text_interpolates_local_fields_format_specs_and_escaped_braces() {
    struct Task {
        title: &'static str,
    }
    let task = Task {
        title: "Build Omen",
    };
    let count = 7;
    let root = view! { Text("{{task}} {task.title} · {count:03}") }.into_node();
    assert!(find(&root, "{task} Build Omen · 007").is_some());
}
