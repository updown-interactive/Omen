use omen_ui::{view, View};
mod views {
    use omen_ui::view;
    view! { pub Header(title: String, subtitle: String) {
        let combined = format!("{title}: {subtitle}");
        Layout { Text(combined) }
    } }
    view! { pub HomeScreen() { Header(subtitle: "world", title: "Hello") } }
}
use views::HomeScreen;
view! { Private(label: String, completed: bool) {
    let items = [label];
    Layout {
        if completed { for item in items { Text(item) } }
        HomeScreen()
    }
} }
fn main() { let _ = view! { Private(completed: true, label: "Test") }.into_node(); }
