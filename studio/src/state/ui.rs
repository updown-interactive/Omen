//! View-owned transient state; never serialized with a project or preferences.
use omen::state;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StudioMenu {
    #[default]
    Closed,
    File,
    Window,
    Run,
}

state! {
    #[derive(Clone, Debug)]
    pub ToolbarState {
        active_menu: StudioMenu = StudioMenu::Closed;

        pub fn toggle(&mut self, menu: StudioMenu) {
            self.active_menu = if self.active_menu == menu { StudioMenu::Closed } else { menu };
        }
    }
}
