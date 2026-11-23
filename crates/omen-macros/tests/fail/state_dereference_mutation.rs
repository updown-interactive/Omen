use omen_ui::{state, view};
state! { Data { items: Box<Vec<i32>> = Box::new(vec![1]) } }
view! { Screen { @State data = Data() Button("Mutate") { (*data.items).push(2); } } }
fn main() {}
