use crate::editor::RenameDialog;
use crate::{app::StudioEnvironment, editor::EditorWorkspace};
use omen::view;
mod agent;
mod appearance;
pub(crate) mod menu;
mod topbar;
pub use topbar::StudioTopbar;
mod welcome;
pub use welcome::StudioLauncher;
mod new_project;
use new_project::ProjectCreator;
mod component_library;
mod debug_console;
mod entry;
mod explorer;
mod file_palette;
use component_library::ComponentLibrary;
use debug_console::DebugConsole;
pub mod inspector;
mod status;
use entry::NewEntryDialog;
mod run;
mod sidebar;
use file_palette::CreationPalette;
use inspector::CodeInspector;
use sidebar::{ACTIVITY_BAR_WIDTH, ActivityBar, Sidebar};
use status::StudioStatusBar;

mod explorer_context;
mod reload;
use explorer_context::{ConfirmTrash, ContextMenu};
use reload::ConfirmReload;
view! {
    pub(super) StudioShell {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        @Environment viewport
        let layout = app.layout;
        let content_width = (viewport.width - ACTIVITY_BAR_WIDTH - 1.).max(0.);
        let geometry = layout.geometry(content_width);
        Layout {
            Layout {
                ActivityBar()
                Layout {}
                    .frame(1, .infinity)
                    .background(colors.outline)
                if layout.sidebar_visible {
                    Layout { Sidebar() }
                    .frame(geometry.sidebar_width, .infinity)
                    .key("sidebar-pane")
                    Layout { Layout {}
                        .frame(1, .infinity)
                        .background(colors.outline) }
                    .frame(6, .infinity)
                    .alignment(.center)
                    .background(colors.surface_container)
                    .resizeHandle(app.visible_width(true, content_width), 180., geometry.sidebar_limit, false)
                    .key("sidebar-split")
                }
                EditorWorkspace(width: geometry.editor_width)
                    .fill()
                    .frame(.infinity, .infinity)
                if layout.preview_visible {
                    Layout { Layout {}
                        .frame(1, .infinity)
                        .background(colors.outline) }
                    .frame(6, .infinity)
                    .alignment(.center)
                    .background(colors.surface_container)
                    .resizeHandle(app.visible_width(false, content_width), 280., geometry.preview_limit, true)
                    .key("preview-split")
                    Layout { CodeInspector() }
                    .frame(geometry.preview_width, .infinity)
                    .key("preview-pane")
                }
            }
            .mode(.horizontal)
            .fill()
            .frame(.infinity, .infinity)
            if app.console_visible {
                Divider()
                DebugConsole()
            }
            Divider()
            StudioStatusBar()
        }
        .frame(.infinity, .infinity)
        .theme(settings.theme())
        .onWindowBlur(move || app.window_blur())
        .background(colors.surface)
        .color(colors.on_surface)
        .shortcut(if cfg!(target_os = "macos") { "cmd-s" } else { "ctrl-s" }, move || app.save_active())
        .shortcut(if cfg!(target_os = "macos") { "cmd-b" } else { "ctrl-b" }, move || app.update_layout(|layout| layout.sidebar_visible = !layout.sidebar_visible))
        .shortcut(if cfg!(target_os="macos") {"cmd-shift-e"} else {"ctrl-shift-e"}, move || app.select_sidebar(crate::state::SidebarPane::Explorer))
        .shortcut(if cfg!(target_os="macos") {"cmd-shift-f"} else {"ctrl-shift-f"}, move || app.select_sidebar(crate::state::SidebarPane::Search))
        .shortcut(if cfg!(target_os="macos") {"cmd-shift-g"} else {"ctrl-shift-g"}, move || app.select_sidebar(crate::state::SidebarPane::SourceControl))
        .shortcut(if cfg!(target_os="macos") {"cmd-i"} else {"ctrl-i"},move ||app.focus_inspector())
        .shortcut(if cfg!(target_os="macos") {"cmd-1"} else {"ctrl-1"},move ||app.focus_editor())
        .shortcut(if cfg!(target_os="macos") {"cmd-alt-1"} else {"ctrl-alt-1"}, move || app.set_visual_mode(crate::visual::EditorMode::Design))
        .shortcut(if cfg!(target_os="macos") {"cmd-alt-2"} else {"ctrl-alt-2"}, move || app.set_visual_mode(crate::visual::EditorMode::Code))
        .shortcut(if cfg!(target_os="macos") {"cmd-alt-3"} else {"ctrl-alt-3"}, move || app.set_visual_mode(crate::visual::EditorMode::Split))
        .context_menu($app.context_visible, move || menu::panel(view! { ContextMenu() }))
        .dialog($app.rename_visible, move || view! { RenameDialog() })
        .dialog($app.reload_visible, move || view! { ConfirmReload() })
        .dialog($app.creation_visible, move || view! { ProjectCreator() })
        .dialog(app.entry_dialog_visibility(), move || view! { NewEntryDialog() })
        .dialog(app.delete_dialog_visibility(), move || view! { ConfirmTrash() })
        .dialog($app.library_visible, move || view! { ComponentLibrary() })
        .dialog($app.palette_visible, move || view! { CreationPalette() })
        .shortcut(if cfg!(target_os="macos") {"cmd-shift-p"} else {"ctrl-shift-p"},move ||app.open_file_chooser())
    }
}

view! {
    /// A standalone workspace owns its event task; the startup router uses StudioShell.
    pub Studio {
        @Environment app: StudioEnvironment
        StudioShell()
            .task(move || app.receive())
            .onWindowClose(move || app.window_close())
    }
}
