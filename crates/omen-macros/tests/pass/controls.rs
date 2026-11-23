use omen_ui::{view, View, TextValue, BoolValue, LinearGradient, Color};
fn main() {
    let text = TextValue::new(String::new());
    let checked = BoolValue::new(false);
    let _ = view! { Card {
        Text("Title").font(.headline).fontSize(22).lineHeight(28).fontFamily("Arial")
            .weight(.semibold).accessibilityRole(.heading).accessibilityHint("Section title")
        Image("sample.png").frame(100, 80).accessibilityLabel("Sample")
        Icon("search").frame(24, 24).color(.purple)
        TextField("Name").value(text).onChange(|| {})
        SecureField("Password").accessibilityLabel("Account password")
        Checkbox("Accept").checked(checked).onChange(|| {})
        Toggle("Notify").onChange(|| {})
        Progress(0.5).color(.green)
        Divider()
        Layout {}.background(LinearGradient::new(Color::Blue, Color::Purple, 90.))
    }.spacing(12).frame(300, 500).alignment(.topLeading).gradient(.blue, .purple, 90).focusable(true) }.into_node();
}
