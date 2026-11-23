use omen_ui::{view, View};
view! { pub Greeting(name: String) { Text(name) } }
fn main() { let _ = view! { Greeting(name: "Hello") }.into_node(); }
