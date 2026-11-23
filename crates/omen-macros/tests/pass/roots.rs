use omen_ui::{view, View};
fn main() {
    let _ = view! { Text("a") Text("b") }.into_node();
    let _ = view! { if false { Text("a") } }.into_node();
    let _ = view! { for item in ["a"] { Text(item) } }.into_node();
}
