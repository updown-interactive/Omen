use omen_ui::view;
fn main() {
    let _ = view! { Layout { Unknown("a") Text("b").font(.nope).sparkle(1) } };
}
