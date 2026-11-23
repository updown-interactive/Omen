use omen::view;

view! {
    pub Header(title: String, subtitle: String) {
        Layout {
            Text(title).font(.largeTitle).weight(.bold)
            Text(subtitle).font(.body).color(.secondary)
        }.spacing(8)
    }
}
