use omen_ui::view;
view! { pub Header(title: String) { Text(title) } }
fn main() { let _ = view! { Header(title: false) }; }
