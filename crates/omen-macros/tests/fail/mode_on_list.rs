use omen_ui::view;
fn main() {
    let _ = view! { List(["a"]) { item in Text(item) }.mode(.stack) };
}
