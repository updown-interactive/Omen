use omen_ui::{state, view};
state! { Data { count = 0 } }
view! { Screen { @State data = Data() Button("Mutate") { let value = &mut data.count; *value += 1; } } }
fn main() {}
