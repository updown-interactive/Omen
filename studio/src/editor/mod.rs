//! Native editor workspace composed with the Omen DSL.
use crate::strings::StudioStrings;
pub(crate) mod hover;
pub mod library;
pub mod lsp;
mod tools;
use crate::{app::StudioEnvironment, state::Document, workspace::Command};
use crate::{
    visual::{EditorMode, VisualCanvas, VisualToolbar},
    workspace::file_kind::StudioFileKind,
};
use omen::{EditorDocument, view};
use tools::FindBar;
pub use tools::{GoToLineDialog, RenameDialog};

view! {
    pub EditorWorkspace(width: f32) {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        @Environment viewport
        let colors = app.colors();
        let state = app.workspace;
        let document = state.active_document().cloned();
        let editor = document.as_ref().map(|document| app.editor_document(document.path.clone()));
        let editing = editor.as_ref().map(|editor| editor.state.get()).unwrap_or_default();
        let breadcrumb = document.as_ref().map(|document| {
            state.root.as_ref()
                .and_then(|root| document.path.strip_prefix(root).ok())
                .unwrap_or(&document.path)
                .components().map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>().join("  ›  ")
        }).unwrap_or_default();
        let visual = if let Some(document) = document.as_ref() {
            app.has_visual_preview(&document.path, &editing.text)
        } else { false };
        let default_mode = (app.settings).design.initial_mode;
        let overwrite_path = document.as_ref().map(|d| d.path.clone()).unwrap_or_default();
        let mode = if visual {
            document.as_ref().and_then(|document| state.canvas(&document.path)).map_or(default_mode, |canvas| canvas.mode)
        } else { EditorMode::Code };
        Layout {
            Layout {
                for document in state.documents {
                    FileTab(
                        document: document.clone(),
                        selected: Some(&document.path) == state.active_file.as_ref()
                    )
                    .key(document.path.display().to_string())
                }
            }
            .mode(.horizontal)
            .spacing(1)
            .frame(.infinity, crate::theme::PANEL_HEADER_HEIGHT - 1.)
            .scrollableHorizontal()
            .background(colors.surface_container)
            Divider()
            if let Some(document) = document {
                if !visual {
                    Layout {
                        Icon("file-code").frame(13,13).color(colors.on_surface_variant)
                        Text(breadcrumb.clone()).fontSize(11).color(colors.on_surface_variant).truncate()
                    }
                    .mode(.horizontal)
                    .spacing(7)
                    .padding(7)
                    .frame(.infinity, 29)
                    .background(colors.surface_container)
                }
                if document.conflict {
                    Layout {
                        Icon("info").frame(13, 13).color(.orange)
                        Text(strings.changed_on_disk_save_is_blocked_to)
                            .color(.orange)
                            .fontSize(12)
                            .truncate()
                        Button(strings.overwrite_disk) {
                            app.send(Command::Overwrite(overwrite_path.clone()));
                        }
                        .background(colors.surface_container_high)
                        .padding(4)
                        Button(strings.reload_from_disk) {
                            app.reload_visible = true;
                        }
                        .padding(4)
                    }
                    .mode(.horizontal)
                    .spacing(8)
                    .padding(7)
                    .frame(.infinity, 29)
                    .background(colors.surface_container)
                }
                if visual {
                    VisualToolbar(path: document.path.clone(), mode: mode, breadcrumb: breadcrumb.clone())
                    Divider()
                }
                Layout {
                    if mode != EditorMode::Code {
                        VisualCanvas(path: document.path.clone(), text: editing.text.clone(), cursor: editing.selection.end, size: [if mode == EditorMode::Split { width * 0.5 } else { width }, (viewport.height - 220.).max(80.)])
                            .key(document.path.display().to_string())
                            .fill()
                    }
                    if mode == EditorMode::Split {
                        Layout {}.frame(1, .infinity).background(colors.outline)
                    }
                    if mode != EditorMode::Design {
                        if let Some(editor) = editor {
                            CodePane(editor: editor.clone(), path: document.path.clone())
                                .fill()
                        }
                    }
                }
                .mode(.horizontal)
                .spacing(0)
                .frame(.infinity, .infinity)
                .fill()

            } else {
                Layout {
                    Text(strings.no_open_documents)
                        .fontSize(18)
                        .weight(.semibold)
                    Text(strings.select_a_file_in_the_explorer_to)
                        .color(colors.on_surface_variant)
                }
                .spacing(16)
                .padding(32)
                .fill()
                .scrollable()
            }
        }
        .spacing(0)
        .fill()
        .frame(.infinity, .infinity)
        .background(colors.surface)
        .color(colors.on_surface)
    }
}

view! {
    CodePane(editor: EditorDocument, path: std::path::PathBuf) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let preferences = (app.settings).editor;
        let editing = editor.state.get();
        let cursor_line = editing.text.byte_to_line(editing.selection.end);
        let cursor_column = editing.text.byte_to_utf16(editing.selection.end) - editing.text.byte_to_utf16(editing.text.line_start(cursor_line));
        Layout {
                if app.find_visible { FindBar() }
                TextEditor("Code editor")
                    .fontFamily("monospace")
                    .fontSize(preferences.font_size)
                    .lineHeight(preferences.line_height)
                    .document(editor.clone())
                    .background(colors.surface)
                    .color(colors.on_surface)
                    .fill()
                    .frame(.infinity, .infinity)
                    .key(path.display().to_string())
                    .accessibilityHint("Rust code editor with line numbers. Control Space completes, F12 goes to definition. Standard selection, clipboard and undo shortcuts are supported.")
                    .shortcut(if cfg!(target_os="macos") {"cmd-l"} else {"ctrl-l"}, move || app.open_component_library())
                if app.problems_visible && !editing.diagnostics.is_empty() {
                    Divider()
                    Layout {
                        for (index, label, is_error) in editing.diagnostics.iter().enumerate().map(|(index, diagnostic)| {
                            let row = editing.text.byte_to_line(diagnostic.range.start) + 1;
                            let col = editing.text.byte_to_utf16(diagnostic.range.start) - editing.text.byte_to_utf16(editing.text.line_start(row - 1)) + 1;
                            (index, format!("{}:{row}:{col} · {}", if diagnostic.error { "Error" } else { "Warn" }, diagnostic.message), diagnostic.error)
                        }) {
                            Button(label) {
                                app.reveal_diagnostic(index);
                            }
                                .alignment(.leading)
                                .frame(.infinity, 24)
                                .padding(4)
                                .fontSize(11)
                                .truncate()
                                .color(if is_error { omen::Color::Red } else { omen::Color::Orange })
                                .hoverBackground(colors.selection)
                        }
                    }
                    .spacing(2)
                    .padding(4)
                    .frame(.infinity, 110)
                    .scrollable()
                    .background(colors.surface_container)
                }
                Layout {
                    Text(format!("Ln {}, Col {} · {}{}", cursor_line+1, cursor_column+1, if editing.secondary_selections.is_empty() {String::new()} else {format!("{} cursors · ",editing.secondary_selections.len()+1)}, app.language_status))
                        .font(.caption)
                        .color(colors.on_surface_variant)
                        .truncate()
                        .fill()
                    Button(format!("{} diagnostics", editing.diagnostics.len())) {
                        app.problems_visible = !app.problems_visible;
                    }
                        .fontSize(11)
                        .padding(3)
                        .cornerRadius(4)
                        .color(if app.problems_visible { colors.accent_foreground } else { colors.on_surface_variant })
                        .hoverBackground(colors.surface_container_high)
                        .accessibilityLabel("Toggle diagnostics panel")
                }
                .mode(.horizontal)
                .spacing(8)
                .padding(4)
                .frame(.infinity, 24)
                .background(colors.surface_container)
        }
        .spacing(0)
        .fill()
        .frame(.infinity, .infinity)
        .shortcut(if cfg!(target_os="macos") {"cmd-f"} else {"ctrl-f"}, move || app.open_find())
        .shortcut(if cfg!(target_os="macos") {"cmd-alt-f"} else {"ctrl-h"}, move || app.open_replace())
        .shortcut("f3", move || app.find_step_sync(true))
        .shortcut("shift-f3", move || app.find_step_sync(false))
        .shortcut(if cfg!(target_os="macos") {"cmd-g"} else {""}, move || if cfg!(target_os="macos") { app.find_step_sync(true); })
        .shortcut(if cfg!(target_os="macos") {"cmd-shift-g"} else {""}, move || if cfg!(target_os="macos") { app.find_step_sync(false); })
        .shortcut("ctrl-g", move || app.open_go_to_line())
        .shortcut("f8", move || app.next_diagnostic(true))
        .shortcut("shift-f8", move || app.next_diagnostic(false))
        .shortcut(if cfg!(target_os="macos") {"cmd-="} else {"ctrl-="}, move || app.zoom_editor(1))
        .shortcut(if cfg!(target_os="macos") {"cmd--"} else {"ctrl--"}, move || app.zoom_editor(-1))
        .shortcut(if cfg!(target_os="macos") {"cmd-0"} else {"ctrl-0"}, move || app.zoom_editor(0))
    }
}

view! {
    FileTab(document: Document, selected: bool) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let select_path = document.path.clone();
        let close_path = document.path.clone();
        let label = document.tab_title();
        let kind = StudioFileKind::classify(&document.path, false);
        let tint = omen::Color::Rgb(kind.tint(app.settings.get().appearance.mode == crate::settings::AppearanceMode::Dark));
        let close_label = format!("Close {}", document.path.display());
        let key = document.path.display().to_string();
        Layout {
            Icon(kind.icon()).frame(15,15).color(tint)
            Button(label) { app.send(Command::Select(select_path.clone())); }
                .color(if selected { colors.on_surface } else { colors.on_surface_variant })
                .weight(if selected { omen::Weight::Semibold } else { omen::Weight::Regular })
                .truncate().fill()
            Button("×") { app.send(Command::Close(close_path.clone())); }
                .frame(22,22).cornerRadius(5)
                .color(colors.on_surface_variant).hoverBackground(colors.surface_container_high)
                .accessibilityLabel(close_label)
        }
        .mode(.horizontal)
        .spacing(6)
        .padding(7)
        .fontSize(12)
        .frame(186, crate::theme::PANEL_HEADER_HEIGHT - 1.)
        .minWidth(140)
        .background(if selected { colors.surface } else { colors.surface_container })
        .key(key)
    }
}
