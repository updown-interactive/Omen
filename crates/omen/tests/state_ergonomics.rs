use omen::prelude::*;
use std::{cell::Cell, rc::Rc};

state! {
    #[derive(Debug)]
    pub CounterState {
        count = 0
        enabled = true
        title = "Hello"
        selected: String = "home"
        opacity: f32 = 1.0
        ratio = 0.5
        small = 2_u8
        negative = -3
        marker = 'x'
        items: Vec<i32> = vec![1, 2]
        index: usize = 1

        pub fn increment(&mut self) { self.count += 1; }
        pub fn toggle(&mut self) { self.enabled.toggle(); }
        pub fn add(&mut self, amount: i32) { self.count += amount; }
    }
}
state! {
    LegacyState {
        let count = 4;
        @State let title: String = "legacy";
    }
}
view! {
    Counter {
        @State state = CounterState()
        Layout {
            Text("Count: {state.count}")
            Text("Enabled: {state.enabled}")
            Text("Selected: {state.selected}")
            Text("Items: {state.items:?}")
            Button("Increment") { state.increment() }
            Button("Toggle") { state.toggle() }
            Button("Reset") { state.count = 0 }
            Button("Legacy update") { state.update(CounterState::increment) }
            Button("Batch") {
                state.count += 1;
                state.increment();
                state.enabled.toggle();
                state.count = state.count + 1;
            }
            Button("Self argument") { state.add(state.count) }
            Button("Indexed mutation") {
                (state.items)[state.index] += state.count;
                state.items[state.index] = state.items[state.index] + 1;
            }
            Button("Shadowed local") {
                let mut state = CounterState::new();
                state.increment();
                assert_eq!(state.count, 1);
            }
            Text("Tap").onTap { state.increment() }
            Text("Closure").onTap(|| { state.increment(); })
            TabView($state.selected) {
                Tab("home", "Home") { Text("Home content") }
                Tab("details", "Details") { Text("Details content") }
            }
            Toggle("Enabled binding", $state.enabled)
        }.spacing(.md).padding(.lg)
    }
}
fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
    if matches!(&node.kind,ViewKind::Text(text)|ViewKind::Button{label:text,..}|ViewKind::Toggle{label:text,..} if text==label)
    {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Layout(children) | ViewKind::Group(children) | ViewKind::Card(children) => {
            children.iter().find_map(|child| find(child, label))
        }
        _ => None,
    }
}
fn click(root: &ViewNode, label: &str) {
    let node = find(root, label).unwrap_or_else(|| panic!("missing {label}"));
    if let Some(action) = &node.style.on_tap {
        action.invoke();
    } else if let ViewKind::Button { action, .. } = &node.kind {
        action.invoke();
    } else {
        panic!("not actionable: {label}");
    }
}
#[test]
fn constructors_defaults_and_literal_types_are_concrete() {
    let state = CounterState::default();
    let _: i32 = state.count;
    let _: bool = state.enabled;
    let _: String = state.title;
    let _: String = state.selected;
    let _: f32 = state.opacity;
    let _: f64 = state.ratio;
    let _: u8 = state.small;
    let _: i32 = state.negative;
    let _: char = state.marker;
    let mut legacy = LegacyState::new();
    legacy.count += 1;
    assert_eq!((legacy.count, legacy.title.as_str()), (5, "legacy"));
}
#[test]
fn direct_actions_bindings_and_rebuilds_share_one_state_instance() {
    let mut runtime = Runtime::new(view! {Counter()}.into_node());
    let root = runtime.render().unwrap();
    click(&root, "Increment");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Count: 1").is_some());
    click(&root, "Toggle");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Enabled: false").is_some());
    find(&root, "Enabled binding")
        .unwrap()
        .style
        .checked
        .as_ref()
        .unwrap()
        .set(true);
    click(&root, "Legacy update");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Count: 2").is_some());
    assert!(find(&root, "Enabled: true").is_some());
    click(&root, "Details");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Selected: details").is_some());
    assert!(find(&root, "Count: 2").is_some());
    click(&root, "Tap");
    click(&root, "Closure");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Count: 4").is_some());
    click(&root, "Self argument");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Count: 8").is_some());
    click(&root, "Indexed mutation");
    let root = runtime.render().unwrap();
    assert!(find(&root, "Items: [1, 11]").is_some());
    click(&root, "Shadowed local");
    assert!(find(&runtime.render().unwrap(), "Count: 8").is_some());
    click(&root, "Reset");
    assert!(find(&runtime.render().unwrap(), "Count: 0").is_some());
}
#[test]
fn multiple_mutations_notify_once_after_the_action() {
    let mut runtime = Runtime::new(view! {Counter()}.into_node());
    let root = runtime.render().unwrap();
    let wakeups = Rc::new(Cell::new(0));
    let observed = wakeups.clone();
    runtime.set_waker(move || observed.set(observed.get() + 1));
    click(&root, "Batch");
    assert_eq!(wakeups.get(), 1);
    let root = runtime.render().unwrap();
    assert!(find(&root, "Count: 3").is_some());
    assert!(find(&root, "Enabled: false").is_some());
}
