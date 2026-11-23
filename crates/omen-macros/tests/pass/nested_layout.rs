use omen_ui::{view, View, LayoutMode};
fn main() {
    let _ = view! { Layout {
        Layout { Text("a") Text("b") }.mode(.horizontal)
        Layout { Text("bottom") Text("top") }.mode(.stack)
        Layout {}.mode(LayoutMode::Vertical)
    }.mode(.vertical) }.into_node();
}
