use omen::prelude::*;
use omen_studio::{
    app::StudioEnvironment,
    views::{StudioLauncher, StudioTopbar},
};

main! { studio_entry() }

fn studio_entry() -> omen::AnyView {
    omen_studio::services::runner::run_child_if_requested();
    omen::AnyView::new(
        view! { StudioLauncher().environment(StudioEnvironment::launch())
        .desktopWindow(DesktopWindow {
            title: "Omen Studio".into(), width: 1280., height: 800.,
            min_width: 820., min_height: 540.,
            toolbar: Some(DesktopToolbar::new(|| StudioTopbar {})),
        }) }
        .into_node(),
    )
}
