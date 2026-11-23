use omen::prelude::*;

mod arbitrary_logic_name {
    use omen::{environment, locale, state};

    state! {
        pub DashboardData {
            let tab: String = "overview";
            let count = 0;
            pub fn increment(&mut self) { self.count += 1; }
        }
    }
    environment! {
        pub Preferences {
            @Environment let count = 0;
            pub fn increment(&mut self) { self.count += 1; }
        }
    }
    locale! {
        pub Messages {
            en { title: "Dashboard" }
            ar { title: "لوحة التحكم" }
        }
    }
}
use arbitrary_logic_name::{DashboardData, Messages, Preferences};
view! {
    SharedReader {
        @Environment preferences: Preferences
        Text("Shared reader: {preferences.count}")
    }
}
view! {
    Screen {
        @State let data = DashboardData::new()
        @Localize strings = Messages::new()
        @Environment preferences: Preferences
        Layout {
            Text(strings.title)
            Text("Translated: {strings.title}")
            Text("{data.tab}: {data.count}")
            TextField("Tab").value($data.tab)
            Button("Increment") { data.update(DashboardData::increment) }
            Button("Shared increment") { preferences.update(Preferences::increment) }
            Button("Arabic") { strings.locale = "ar" }
            SharedReader()
        }.locale(strings.locale)
    }
}
fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind,ViewKind::Text(text)|ViewKind::Button{label:text,..}|ViewKind::TextField{label:text,..} if text==label)
    {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Layout(children) | ViewKind::Group(children) => {
            children.iter().find_map(|child| find(child, label))
        }
        _ => None,
    }
}
fn click(root: &ViewNode, label: &str) {
    let ViewKind::Button { action, .. } = &find(root, label).unwrap().kind else {
        panic!("not a button")
    };
    action.invoke();
}
#[test]
fn imported_logic_models_bind_and_invalidate_views() {
    let preferences = Preferences::shared();
    let mut runtime = Runtime::new(
        view! {Screen()}
            .environment(preferences.clone())
            .into_node(),
    );
    let root = runtime.render().unwrap();
    assert!(find(&root, "overview: 0").is_some());
    click(&root, "Increment");
    let root = runtime.render().unwrap();
    assert!(find(&root, "overview: 1").is_some());
    find(&root, "Tab")
        .unwrap()
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("details".into());
    click(&root, "Shared increment");
    click(&root, "Arabic");
    let root = runtime.render().unwrap();
    assert!(find(&root, "details: 1").is_some());
    assert!(find(&root, "Shared reader: 1").is_some());
    assert!(find(&root, "لوحة التحكم").is_some());
    assert!(find(&root, "Translated: لوحة التحكم").is_some());
    assert_eq!(preferences.get().count, 1);
}
#[test]
fn catalogs_create_independent_locale_selection() {
    let first = Messages::new();
    let second = Messages::new();
    first.set_locale(Locale::new("ar"));
    assert_eq!(first.text("title"), "لوحة التحكم");
    assert_eq!(second.text("title"), "Dashboard");
}
