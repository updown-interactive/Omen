use omen::{Action, DesktopToolbar, DesktopWindow, main, view};
use omen_components::{
    EmptyState, FormRow, LoadingCard, PreferenceRow, PrimaryButton, SectionHeader, StatusBadge,
};

view! {
    pub Gallery {
        @State let name = String::new()
        @State let password = String::new()
        @State let accepted = false
        @State let notifications = true
        let submit = Action::new(move || println!("Form action activated"));
        let picture = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sample.png");
        Layout {
            SectionHeader(title: "Omen view library", subtitle: "Native controls • keyboard focus • accessible semantics")
            Layout {
                Card {
                    Text("Form controls")
                    .font(.headline)
                    .accessibilityRole(.heading)
                    FormRow(label: "Name", value: $name)
                    SecureField("Password")
                    .value($password).
                    accessibilityHint("Your input stays masked")
                    Checkbox("Accept terms")
                    .checked($accepted)
                    PreferenceRow(title: "Notifications", value: $notifications)
                    PrimaryButton(title: "Submit", action: submit)
                    Divider()
                    Text("Use Tab and Shift-Tab to move focus")
                    .font(.caption)
                    .color(.secondary)
                }.frame(340, 520)
                Card {
                    Text("Content and appearance").font(.headline).accessibilityRole(.heading)
                    Image(picture).frame(250, 130).accessibilityLabel("Blue and teal gradient sample")
                    Layout {
                        Icon("check").color(.green).accessibilityLabel("Complete")
                        Icon("search").color(.blue).accessibilityLabel("Search")
                        Icon("settings").color(.secondary).accessibilityLabel("Settings")
                        Icon("plus").color(.purple).accessibilityLabel("Add")
                    }.mode(.horizontal).spacing(12)
                    StatusBadge(label: "Ready")
                    Progress(0.65).color(.green).accessibilityLabel("Project completion")
                    Text("Typography").fontFamily("Arial").fontSize(22).lineHeight(28).weight(.light)
                    Text("Two-color gradient").padding(10).gradient(.blue, .purple, 90).cornerRadius(8)
                }.frame(340, 520)
            }.mode(.horizontal).spacing(16)
            Layout {
                LoadingCard(title: "Preparing your workspace", fraction: 0.75_f32)
                EmptyState(title: "All caught up", message: "New items will appear here")
            }.mode(.horizontal).spacing(16)
            List(["Text and images", "Forms and actions", "Cards and progress"]) { item in
                Text(item).font(.footnote).color(.secondary)
            }.spacing(6).accessibilityLabel("View categories")
        }.spacing(20).padding(24).background(.dark).color(.white)
    }
}
view! {
    GalleryToolbar {
        @State let count = 0
        Layout {
            Text("Omen Gallery").weight(.semibold)
            Layout {}.fill()
            Button("Toolbar clicks: {count}") { count += 1; }
        }.mode(.horizontal).spacing(16).padding(8).frame(.infinity, .infinity)
    }
}
main! {
    Gallery().desktopWindow(DesktopWindow {
        title: "Omen Gallery".into(),
        toolbar: Some(DesktopToolbar::new(|| GalleryToolbar {})),
        ..Default::default()
    })
}
