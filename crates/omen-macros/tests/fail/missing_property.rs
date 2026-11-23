use omen_ui::view;
view! { pub Header(title: String, subtitle: String) { Text(title) Text(subtitle) } }
fn main() { let _ = view! { Header(title: "a") }; }
