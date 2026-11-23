use omen::prelude::*;

environment! {
    pub AppEnvironment {
        @Reactive username = "Guest"
        @Reactive dark = false
        @Reactive count: usize = 0
        pub(crate) navigator: Navigator = Navigator::new()
        receiver: std::sync::mpsc::Receiver<()> = std::sync::mpsc::channel().1
        pub fn increment(&self) { self.count.update(|count| *count += 1); }
    }
}
impl AppEnvironment {
    fn greeting(&self) -> String {
        self.username.get()
    }
}
locale! {
    pub AppStrings {
        default: "en"
        en {
            welcome: "Welcome"
            greeting: "Hello {name}"
            files: { one: "{count} file", other: "{count} files" }
            fallback: "Fallback"
        }
        ar {
            welcome: "مرحباً"
            greeting: "أهلاً {name}"
            files: { zero: "No files", one: "One file", two: "Two files", few: "{count} few", many: "{count} many", other: "{count} files" }
        }
    }
}
view! {
    Home {
        @Environment app
        @Localize strings = AppStrings()
        Layout {
            Text(strings.welcome)
            Text(app.username)
            Text(app.greeting())
            TextField("Username").value($app.username)
            Button("Toggle theme") { app.dark.toggle() }
            Button("Increment") { app.increment() }
            Button("Reset") { app.username = "Guest" }
            Button("Arabic") { strings.locale = "ar" }
        }.theme(if app.dark { .dark } else { .light })
    }
}
fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind, ViewKind::Text(text) | ViewKind::Button {label:text,..} | ViewKind::TextField {label:text,..} if text == label)
    {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Layout(children) | ViewKind::Group(children) => {
            children.iter().find_map(|node| find(node, label))
        }
        _ => None,
    }
}
fn click(node: &ViewNode, label: &str) {
    let ViewKind::Button { action, .. } = &find(node, label).unwrap().kind else {
        panic!("button missing")
    };
    action.invoke();
}
#[test]
fn reactive_resources_retain_ownership_and_invalidate_views() {
    let app = Value::new(AppEnvironment::new());
    let mut runtime = Runtime::new(view! { Home() }.environment(app.clone()).into_node());
    let first = runtime.render().unwrap();
    assert!(find(&first, "Guest").is_some());
    find(&first, "Username")
        .unwrap()
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("Ada".into());
    click(&first, "Toggle theme");
    click(&first, "Increment");
    click(&first, "Arabic");
    let second = runtime.render().unwrap();
    assert!(find(&second, "Ada").is_some());
    assert!(find(&second, "مرحباً").is_some());
    assert!(app.read(|model| model.dark.get()));
    assert_eq!(app.read(|model| model.count.get()), 1);
    assert!(app.read(|model| model.receiver.try_recv().is_err()));
    let _ = app.read(|model| model.navigator.clone());
    let third = runtime.render().unwrap();
    assert!(find(&third, "مرحباً").is_some());
    click(&third, "Reset");
    assert!(find(&runtime.render().unwrap(), "Guest").is_some());
}
#[test]
fn catalogs_validate_fallback_interpolation_plurals_and_direction() {
    let strings = AppStrings::new();
    assert_eq!(strings.greeting("Ada"), "Hello Ada");
    assert_eq!(strings.files(1), "1 file");
    assert_eq!(strings.files(3), "3 files");
    strings.try_set_locale(Locale::new("ar-EG")).unwrap();
    assert_eq!(strings.locale().direction, LayoutDirection::RightToLeft);
    assert_eq!(strings.files(0), "No files");
    assert_eq!(strings.files(2), "Two files");
    assert_eq!(strings.files(5), "5 few");
    assert_eq!(strings.fallback(), "Fallback");
    assert!(strings.try_set_locale(Locale::new("zz")).is_err());
    assert_eq!(strings.welcome(), "مرحباً");
    assert_eq!(AppStrings::new().welcome(), "Welcome");
}
#[test]
fn generated_environment_can_be_injected_without_a_wrapper() {
    let mut runtime = Runtime::new(
        view! { Home() }
            .environment::<AppEnvironment>(AppEnvironment::new())
            .into_node(),
    );
    assert!(find(&runtime.render().unwrap(), "Welcome").is_some());
}

environment! {
    InjectedResources {
        @Reactive active: bool;
        receiver: std::sync::mpsc::Receiver<()>;
    }
}
#[test]
fn constructor_accepts_resources_without_initializers() {
    let (_, receiver) = std::sync::mpsc::channel();
    let environment = InjectedResources::new(true, receiver);
    assert!(environment.active.get());
    assert!(environment.receiver.try_recv().is_err());
}
#[test]
fn nested_environments_override_without_changing_the_parent() {
    let parent = AppEnvironment::new();
    parent.username.set("Parent".into());
    let child = AppEnvironment::new();
    child.username.set("Child".into());
    let mut runtime = Runtime::new(
        Layout::new()
            .children(vec![
                view! { Home() }.into_node(),
                view! { Home() }.environment(child).into_node(),
            ])
            .environment(parent)
            .into_node(),
    );
    let node = runtime.render().unwrap();
    assert!(find(&node, "Parent").is_some());
    assert!(find(&node, "Child").is_some());
}
