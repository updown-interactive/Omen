use omen::prelude::*;

mod main_state;
mod translations;
use main_state::TaskState;
use translations::AppStrings;

// Supporting declarations can also live alongside their views.
environment! {
    Session {
        @Environment let owner: String = "Guest";
    }
}

view! {
    Home {
        @State model = TaskState()
        @Localize strings = AppStrings::new()
        @Environment session: Session
        @Environment viewport

        Layout {
            Text(strings.welcome).font(.largeTitle)
            Text("Welcome, {session.owner}")
            Text("Tab: {model.tab} · Count: {model.count}")
            TextField("Your name").value($model.name)
            Text("Hello, {model.name}")
            Button("Increment") { model.increment() }
            Button("English") { strings.locale = "en" }
            Button("العربية") { strings.locale = "ar" }
            Text("Viewport: {viewport.breakpoint} · {viewport.width} × {viewport.height}")
                .font(.caption)
                .color(.secondary)
        }
        .spacing(.md)
        .padding(.lg)
        .locale(strings.locale)
    }
}

main! { Home().environment(Session::shared()) }
