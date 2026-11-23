use omen::view;

view! {
    pub TaskCard(title: String, completed: bool) {
        let marker = if completed { "✓" } else { "○" };
        Layout {
            Text(marker).color(.green)
            Text(title).font(.body)
        }.mode(.horizontal).spacing(12).padding(16)
    }
}
