use omen_ui::{view, View, TaskHandle};
#[derive(Clone)]
struct Model { name: String }
view! {
    Counter() {
        @State let count: i32 = 0;
        @State let request: Option<TaskHandle> = None;
        @Environment let model: Model;
        let title = "Counter";
        Layout {
            Text(title)
            Text("Count: {count}")
            TextField("Name").value($model.name)
            Button("Increment") { count += 1; }
            Text("Tap").onTap { count = count + 1; }
            Button("Load") async { count += 1; }.taskHandle($request)
            Button("Cancel") { if let Some(task) = request { task.cancel(); } }
            List(vec![1, 2], id: |id| *id) { id in Text(format!("{id}")) }
        }.onAppear { count += 1; }.task async { count += 1; }
    }
}
fn main() { let _ = view! { Counter() }.into_node(); }
