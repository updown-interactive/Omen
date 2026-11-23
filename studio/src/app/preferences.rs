//! Appearance, persisted layout and panel bindings.
use super::*;

impl StudioEnvironment {
    pub fn update_layout(&self, update: impl FnOnce(&mut LayoutPreferences)) {
        if !self.ready.get() {
            return;
        }
        self.layout.update(|layout| {
            update(layout);
            layout.sanitize();
        });
        if let Some(service) = &self.service
            && let Err(message) = service.layout(self.layout.get())
        {
            self.workspace.update(|state| state.message = message);
        }
    }
    pub fn width(&self, sidebar: bool) -> Binding<f32> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || {
                let layout = read.layout.get();
                if sidebar {
                    layout.sidebar_width
                } else {
                    layout.preview_width
                }
            },
            move |value| {
                write.update_layout(|layout| {
                    if sidebar {
                        layout.sidebar_width = value
                    } else {
                        layout.preview_width = value
                    }
                })
            },
        )
    }
    /// Begin a resize from the displayed size, which may be narrower than the saved preference.
    pub fn visible_width(&self, sidebar: bool, viewport_width: f32) -> Binding<f32> {
        let read = self.clone();
        let write = self.width(sidebar);
        Binding::new(
            move || {
                let geometry = read.layout.get().geometry(viewport_width);
                if sidebar {
                    geometry.sidebar_width
                } else {
                    geometry.preview_width
                }
            },
            move |width| write.set(width),
        )
    }
    pub fn colors(&self) -> StudioColors {
        self.settings.get().colors()
    }
    pub fn update_settings(&self, update: impl FnOnce(&mut StudioSettings)) {
        if !self.ready.get() {
            return;
        }
        let previous = self.settings.get();
        let mut settings = previous.clone();
        update(&mut settings);
        settings.sanitize();
        if settings == previous
            && !self.workspace.read(|state| {
                state.message.contains("Settings recovery:")
                    || state.message.starts_with("Could not save settings.json:")
            })
        {
            return;
        }
        if settings.editor.letter_spacing != previous.editor.letter_spacing {
            for editor in self.editor_states.borrow().values() {
                editor.update(|state| state.letter_spacing = settings.editor.letter_spacing);
            }
        }
        self.settings.set(settings.clone());
        if let Some(service) = &self.service
            && let Err(message) = service.settings(settings)
        {
            self.workspace.update(|state| state.message = message);
        }
    }
    pub(crate) fn save_account(&self, name: &str, email: &str) {
        let Some(service) = &self.service else { return };
        let account = crate::account::Account {
            name: name.trim().to_owned(),
            email: email.trim().to_owned(),
        };
        if let Err(message) = service.account(account) {
            self.workspace
                .update(|workspace| workspace.message = message);
        }
    }

    pub fn set_appearance(&self, mode: AppearanceMode) {
        self.update_settings(|settings| settings.appearance.mode = mode);
    }
    pub fn select_theme(&self, preset: ThemePreset) {
        self.update_settings(|settings| settings.appearance.theme = preset);
    }
    pub(crate) fn design_grid(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.settings.read(|settings| settings.design.show_grid),
            move |enabled| write.update_settings(|settings| settings.design.show_grid = enabled),
        )
    }

    pub fn dark(&self) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.settings.get().appearance.mode == AppearanceMode::Dark,
            move |value| {
                write.set_appearance(if value {
                    AppearanceMode::Dark
                } else {
                    AppearanceMode::Light
                })
            },
        )
    }
}
