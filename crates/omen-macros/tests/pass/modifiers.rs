use omen_ui::{view, View, Color, Font, Weight};
fn main() {
    let _ = view! { Layout { Text("Hello").font(.largeTitle).weight(.bold).color(.white)
        Text("Typed").font(Font::Body).weight(Weight::Medium).color(Color::Secondary)
        Button("Save") {}.padding(16).background(.blue).cornerRadius(12)
    }.spacing(8).padding(4.5_f32).background(.dark) }.into_node();
}
