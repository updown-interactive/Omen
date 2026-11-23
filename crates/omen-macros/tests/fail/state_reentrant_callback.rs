use omen_ui::{state, view};
state! { Data { items: Vec<i32> = vec![1] enabled = true } }
view! { Screen { @State data = Data() Button("Mutate") { data.items.retain(|_| data.enabled); } } }
fn main() {}
