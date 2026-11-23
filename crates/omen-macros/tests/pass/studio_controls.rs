use omen_ui::{View, view};

view! {
    CaptureRegression {
        @State let count = 0
        let path = std::path::PathBuf::from("src/main.rs");
        Button("Open") { count += 1; println!("{}", path.display()); }
    }
}
fn main() {
    let width = omen_ui::Value::new(240_f32);
    let text = omen_ui::TextValue::new("fn main() {}\n".to_string());
    let _ = view! {
        Layout {
            Layout {}.frame(6, 400).resizeHandle(width, 180., 480., false)
            TextEditor("Code").value(text).fill()
            List(0..100) { index in Text("Row {index}") }.virtualized(28.).fill()
            Layout {}.frame(200, 40).scrollableHorizontal().minWidth(180)
            CaptureRegression()
        }.desktopWindow(omen_ui::DesktopWindow {
            toolbar: Some(omen_ui::DesktopToolbar::new(|| view! { Button("Toolbar") {} })),
            ..Default::default()
        })
        .shortcut("cmd-s", move || {})
        .onContextMenu(move || {})
    }.into_node();
}
