use omen::{TaskHandle, main, view};
use std::time::Duration;

view! {
    Counter {
        @State let count = 0
        Layout {
            Text("Count: {count}").font(.title)
            Button("Increment") { count += 1; }
            Button("Reset") { count = 0; }
        }.spacing(12).padding(16).background(.panel).cornerRadius(12)
    }
}

view! {
    Home {
        @State let name = String::new()
        @State let visible = true
        @State let rows = vec![1_u32, 2, 3]
        @State let message = String::from("Ready")
        @State let request: Option<TaskHandle> = None
        let greeting = if name.is_empty() { "world".to_string() } else { name };
        let request_status = request.as_ref().map(TaskHandle::status);
        Layout {
            Text("Reactive Omen").font(.largeTitle)
            TextField("Your name").value($name)
            Text("Hello, {greeting}")
            Toggle("Show counter").checked($visible)
            if visible { Counter() }
            Text(message)
            Text("Request: {request_status:?}").font(.caption)
            Button("Load asynchronously") async {
                message = "Loading…".to_string();
                omen::sleep(Duration::from_secs(2)).await;
                message = "Finished".to_string();
            }.taskHandle($request)
            Button("Cancel request") {
                if let Some(task) = request { task.cancel(); }
                message = "Cancelled".to_string();
            }
            Button("Reverse rows") { rows.reverse(); }
            List(rows, id: |id| *id) { id in
                Layout { Text("Row {id}") Counter() }.spacing(8)
            }
        }.spacing(16).padding(24)
    }
}

main! { Home() }
