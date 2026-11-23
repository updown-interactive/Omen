use omen_ui::{view, View};
fn main() {
    let tasks = ["a", "b"];
    let __omen_children = "hygiene";
    let __omen_rows = ["not captured"];
    let _ = view! { Layout {
        for (index, task) in tasks.iter().enumerate() { Text(format!("{index}: {task}")) Text(__omen_children) }
        for task in __omen_rows { Text(task) }
        List(tasks) { task in Text(task) Text("detail") }.spacing(2)
        List([(1, "one")]) { (_, value) in Text(value) }
        List(Vec::<String>::new()) { value in Text(value) }
    } }.into_node();
}
