use crate::strings::StudioStrings;
use crate::{app::StudioEnvironment, state::TreeRow, workspace::Command};
use omen::view;

#[derive(Clone, Copy)]
enum ExplorerAction {
    CreateNew,
    NewFolder,
    Refresh,
}
impl ExplorerAction {
    fn icon(self) -> &'static str {
        match self {
            Self::CreateNew => "file-plus",
            Self::NewFolder => "folder-plus",
            Self::Refresh => "refresh",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::CreateNew => "Create New",
            Self::NewFolder => "New folder",
            Self::Refresh => "Refresh explorer",
        }
    }
}

view! {
    pub ProjectExplorer {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let state = app.workspace;
        let rows = state.rows;
        let context_root = state.root.clone();
        @State let project_open = true
        Layout {
            Layout {
                Icon("files").frame(15,15).color(colors.on_surface_variant)
                Text(strings.explorer)
                    .fontSize(12)
                    .weight(.medium)
                    .color(colors.on_surface_variant)
            }
            .mode(.horizontal).spacing(7)
            .padding(8)
            .frame(.infinity, crate::theme::PANEL_HEADER_HEIGHT - 1.)
            Divider()
            if let Some(root) = state.root {
                Layout {
                    Layout {
                        Icon(if project_open { "chevron-down" } else { "chevron-right" })
                            .frame(14, 14)
                    }
                    .frame(20, 28)
                    .alignment(.center)
                    .onTap(move || project_open = !project_open)
                    .accessibilityRole(.button)
                    .accessibilityLabel(if project_open { "Collapse project" } else { "Expand project" })
                    Text(root.file_name().unwrap_or_default().to_string_lossy().into_owned())
                        .fontSize(13)
                        .weight(.semibold)
                        .fill()
                        .truncate()
                    ExplorerActionButton(action: ExplorerAction::CreateNew)
                    ExplorerActionButton(action: ExplorerAction::NewFolder)
                    ExplorerActionButton(action: ExplorerAction::Refresh)
                }
                .mode(.horizontal)
                .spacing(3)
                .padding(6)
                .frame(.infinity, 40)
                .onContextMenu(move || {if let Some(path)=context_root.clone() {app.show_context(TreeRow {path,depth:0,directory:true,expanded:true});}})
                if project_open {
                    List(
                        rows,
                        id: |row| row.path.display().to_string()
                    ) { row in
                        ExplorerRow(row: row)
                    }
                    .virtualized(28.)
                    .fill()
                    .frame(.infinity, .infinity)
                }
            } else {
                Text(strings.open_a_project_to_explore_its_files)
                    .font(.caption)
                    .color(colors.on_surface_variant)
                    .padding(12)
            }
        }
        .spacing(0)
        .frame(.infinity, .infinity)
        .background(colors.surface_container)
        .color(colors.on_surface)
    }
}

view! {
    ExplorerActionButton(action: ExplorerAction) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout { Icon(action.icon())
                .frame(17, 17) }
        .frame(26, 28)
        .alignment(.center)
        .cornerRadius(6)
        .hoverBackground(colors.surface_container_high)
        .color(colors.on_surface_variant)
        .onTap(move || match action {
                ExplorerAction::CreateNew => app.open_file_chooser(),
                ExplorerAction::NewFolder => app.begin_new_entry(true),
                ExplorerAction::Refresh => app.send(Command::Refresh),
            })
        .accessibilityRole(.button)
        .accessibilityLabel(action.label())
        .accessibilityHint(action.label())
    }
}

view! {
    ExplorerRow(row: TreeRow) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        let path = row.path.clone();
        let selected_path = row.path.clone();
        let context_row = row.clone();
        let rename_row = row.clone();
        let directory = row.directory;
        let name = row.name();
        let icon = row.icon();
        let file_kind=crate::workspace::file_kind::StudioFileKind::classify(&row.path,row.directory);
        let selected = app.explorer_selection == Some(row.path.clone());
        let accessible = row.path.display().to_string();
        let icon_color = if directory { if selected { colors.on_selection } else { omen::Color::Rgb(0x8096ad) } } else if file_kind!=crate::workspace::file_kind::StudioFileKind::Other {
            omen::Color::Rgb(file_kind.tint(settings.appearance.mode==crate::settings::AppearanceMode::Dark))
        } else {
            match icon {
                "file-rust" => omen::Color::Rgb(0xe97932),
                "file-config" => omen::Color::Rgb(0xc9a652),
                "file-image" => omen::Color::Rgb(0xaa87cf),
                "file-code" => colors.accent_foreground,
                _ => colors.on_surface_variant,
            }
        };
        Layout {
            Layout {}
            .frame(20, 28)
            for _level in 0..row.depth {
                Layout { Layout {}
                    .frame(1, 28)
                    .background(colors.outline) }
                .frame(16, 28)
                .minWidth(16)
                .alignment(.center)
            }
            Layout {
                if directory { Icon(if row.expanded { "chevron-down" } else { "chevron-right" })
                        .frame(12, 12)
                        .color(colors.on_surface_variant) }
            }
            .frame(16, 28)
            .minWidth(16)
            .alignment(.center)
            Icon(icon)
                .frame(16, 16)
                .minWidth(16)
                .color(icon_color)
            Layout {}
            .frame(6, 28)
            Text(name)
                .fontSize(13)
                .fill()
                .alignment(.leading)
                .truncate()
            Layout {}
            .frame(8, 28)
        }
        .mode(.horizontal)
        .alignment(.leading)
        .frame(.infinity, 28)
        .background(if selected { colors.selection } else { colors.surface_container })
        .hoverBackground(if selected { colors.selection } else { colors.surface_container_high })
        .color(if selected { colors.on_selection } else if row.name().starts_with('.') { colors.on_surface_variant } else { colors.on_surface })
        .onTap(move || {
                app.explorer_selection.set(Some(selected_path.clone()));
                app.send(if directory { Command::ToggleDirectory(path.clone()) } else { Command::OpenFile(path.clone()) });
            })
        .onContextMenu(move || app.show_context(context_row.clone()))
        .shortcut("f2", move || {app.show_context(rename_row.clone());app.rename_context();})
        .accessibilityRole(.button)
        .accessibilityLabel(accessible)
        .accessibilityHint(if directory { "Expand or collapse folder" } else { "Open file" })
    }
}
