use omen_ui::{view, View};
fn main() {
    let maybe = Some("value");
    let _ = view! { Layout {
        if let Some(value) = maybe { Text(value) } else { Layout {} }
        if false { Text("hidden") }
        if true { if false { Text("nested") } else { Text("yes") } } else if false { Text("no") }
    } }.into_node();
}
