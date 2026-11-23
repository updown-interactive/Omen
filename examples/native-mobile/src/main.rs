use omen::{main, view};

view! {
    pub NativeHome {
        @State let count = 0_i32
        Layout {
            Text("Omen · Native GPUI").font(.largeTitle)
            Text("Count: {count}").font(.title)
            Button("Increment") { count += 1; }
                .padding(20)
                .background(.accent)
                .color(.white)
                .cornerRadius(12)
            Text("Rust views · native graphics · no WebView")
        }
        .spacing(20)
        .padding(32)
        .safeArea(.all)
    }
}

main! { NativeHome() }
