use omen::{main, view};

main! {
    view! {
        Layout {
            Text("Welcome to Omen").font(.largeTitle)
            Text("A Rust-first UI framework")
        }.spacing(16).padding(32).background(.dark).color(.white)
    }
}
