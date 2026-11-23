//! Keyboard-driven component library with source-owned insertion snippets.
use crate::{app::StudioEnvironment, editor::library::LibraryItem};
use omen::view;
view! {
    pub ComponentLibrary {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let settings = app.settings;
        let query = app.library_query;
        let items: Vec<_> = (app.library_items).into_iter().filter(|item| item.matches(&query)).collect();
        let index = (app.library_index).min(items.len().saturating_sub(1));
        let selected = items.get(index).cloned();
        let first = index.saturating_sub(5);
        Layout {
            Layout {
                Icon("files")
                    .frame(20,20)
                    .color(colors.accent_foreground)
                Text("View Library")
                    .fontSize(18)
                    .weight(.semibold).fill()
                Button("×") {app.library_visible = false;}
                .accessibilityLabel("Close component library")
            }
            .mode(.horizontal)
            .spacing(10)
            Layout {
                Icon("search")
                    .frame(16,16)
                    .color(colors.on_surface_variant)
                TextField("Search views…")
                    .value($app.library_query)
                    .autofocus(true)
                    .onChange(move || app.library_index.set(0))
                    .fontSize(14)
                    .frame(.infinity,34)
            }
            .mode(.horizontal)
            .spacing(8)
            .padding(6)
            .background(colors.surface)
            .cornerRadius(7)
            Divider()
            Layout {
                Layout {
                    Text(format!("VIEWS · {}",items.len()))
                        .fontSize(10)
                        .weight(.semibold)
                        .color(colors.on_surface_variant)
                    for (row,item) in items.iter().enumerate().skip(first).take(6) {
                        LibraryRow(
                            item: item.clone(),
                            index: row,
                            selected: row == index
                        )
                    }
                    if items.is_empty() {
                        Text("No matching views")
                            .fontSize(13)
                            .padding(16)
                            .color(colors.on_surface_variant)
                    }
                    Layout {}
                    .fill()
                }
                .spacing(5)
                .frame(250,.infinity)
                Layout {}
                .frame(1,.infinity)
                .background(colors.outline)
                Layout {
                    if let Some(item) = selected {
                        Text(item.snippet.name.clone())
                            .fontSize(24)
                            .weight(.semibold)
                        Text(item.category)
                            .fontSize(11)
                            .weight(.medium)
                            .color(colors.accent_foreground)
                        Text(item.summary)
                            .fontSize(14)
                            .color(colors.on_surface_variant)
                        Divider()
                        Text("Overview")
                            .fontSize(12)
                            .weight(.semibold)
                        Text(item.documentation)
                            .fontSize(13)
                            .lineHeight(20)
                        Text("Example")
                            .fontSize(12)
                            .weight(.semibold)
                        Text(item.snippet.insert.clone())
                            .fontFamily("monospace")
                            .fontSize(12)
                            .lineHeight(19)
                            .padding(12)
                            .frame(.infinity,.infinity)
                            .background(colors.surface)
                            .color(colors.accent_foreground)
                            .cornerRadius(6)
                        Text("Insert at the editor cursor, then use Tab and Shift-Tab to move through the placeholders.")
                            .fontSize(11)
                            .color(colors.on_surface_variant)
                    } else {
                        Text("Find a view component")
                            .fontSize(18)
                            .weight(.semibold)
                        Text("Search by name, category or purpose.")
                            .fontSize(13)
                            .color(colors.on_surface_variant)
                    }
                }
                .padding(8)
                .spacing(12)
                .fill()
                .frame(.infinity,.infinity)
                .scrollable()
            }
            .mode(.horizontal)
            .spacing(12)
            .fill()
            .frame(.infinity,370)
            Divider()
            if !(app.library_message).is_empty() {Text(app.library_message)
                    .fontSize(12)
                    .color(.orange)}
            Layout {
                Text("↑ ↓ Choose   ↵ Insert   Esc Close   Tab Focus")
                    .fontSize(11)
                    .color(colors.on_surface_variant).fill()
                Button("Insert View") {app.insert_library_selection();}
                .padding(8)
                .fontSize(12)
                .background(colors.primary)
                .color(colors.on_primary)
                .cornerRadius(5)
            }
            .mode(.horizontal)
            .spacing(12)
        }
        .padding(12)
        .spacing(12)
        .frame(850,550)
        .theme(settings.theme())
        .background(colors.surface_container)
        .color(colors.on_surface)
        .shortcut("down",move ||app.library_move(1))
        .shortcut("up",move ||app.library_move(-1))
        .shortcut("home",move ||app.library_index.set(0))
        .shortcut("end",move ||app.library_move(isize::MAX))
        .shortcut("enter",move ||app.insert_library_selection())
        .shortcut("escape",move ||app.library_visible.set(false))
    }
}
view! {
    LibraryRow(item: LibraryItem, index: usize, selected: bool) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            Layout {Icon(item.icon)
                    .frame(20,20)
                    .color(colors.accent_foreground)}
            .frame(32,32)
            .background(colors.surface_container_high)
            .cornerRadius(6)
            Layout {
                Text(item.snippet.name.clone())
                    .fontSize(13)
                    .weight(.medium)
                Text(item.category)
                    .fontSize(10)
                    .color(colors.on_surface_variant)
            }
            .spacing(3).fill()
        }
        .mode(.horizontal)
        .spacing(10)
        .padding(7)
        .frame(.infinity,50)
        .cornerRadius(6)
        .background(if selected {colors.selection} else {colors.surface_container})
        .hoverBackground(colors.surface_container_high)
        .onTap(move ||app.library_index.set(index))
        .accessibilityRole(.button)
        .accessibilityLabel(item.snippet.name.clone())
        .accessibilityHint("Show component documentation; press Enter to insert")
    }
}
