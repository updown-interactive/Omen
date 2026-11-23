use omen_ui::{state, view};
state! { Data { items: Vec<i32> = vec![1] } }
view! { Screen { @State data = Data() Button("Mutate") { data.items.iter_mut().for_each(|item| *item += 1); } } }
fn main() {}
