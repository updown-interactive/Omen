//! File creation and rename forms.
use crate::{app::StudioEnvironment, strings::StudioStrings};
use omen::view;

view! {
    pub NewEntryDialog {
        @Environment app: StudioEnvironment
        @Localize strings = StudioStrings()
        let colors = app.colors();
        let settings = app.settings;
        let draft = app.new_entry;
        let preview = draft.kind.and_then(|kind|crate::workspace::file_kind::FileNames::new(&draft.name,kind).ok()).map(|names|names.file_name);
        let destination = draft.target.and_then(|index|draft.targets.get(index)).map(|target|target.directory.display().to_string());
        Layout {
            Text(if let Some(kind)=draft.kind {kind.label()} else if draft.source.is_some() { "Rename" } else if draft.directory { "New folder" } else { "New file" })
                .fontSize(22)
                .weight(.semibold)
            if let Some(parent) = draft.parent { Text(parent.display().to_string())
                    .font(.caption)
                    .color(colors.on_surface_variant) }
            Text(strings.name)
                .fontSize(12)
                .weight(.semibold)
            Layout {
                TextField(if draft.directory { "Folder name" } else { "File name" })
                    .value(app.entry_name())
                    .autofocus(true)
                    .frame(.infinity, 38)
                    .background(colors.surface)
                    .cornerRadius(7)
            }
            .padding(1)
            .background(colors.outline)
            .cornerRadius(8)
            if draft.kind.is_none() && draft.source.is_none() && !draft.directory && draft.name.ends_with(".rs") {
                Button(strings.enable_rust_module_registration) async {
                    app.new_entry.update(|draft| {draft.kind=Some(crate::workspace::file_kind::StudioFileKind::Rust);draft.register=true;draft.resolving=true;});
                    app.clone().inspect_entry().await;
                }
                .fontSize(12)
                .color(colors.accent_foreground)
            }
            if let Some(name)=preview { Text(format!("Creates {name}"))
                    .font(.caption)
                    .color(colors.on_surface_variant) }
            if draft.register {if let Some(destination)=destination {Text(format!("Module directory: {destination}"))
                        .font(.caption)
                        .color(colors.on_surface_variant)}}
            if draft.resolving { Text(strings.resolving_rust_modules)
                    .font(.caption) }
            if let Some(kind)=draft.kind {
                if kind!=crate::workspace::file_kind::StudioFileKind::Folder {
                    if kind!=crate::workspace::file_kind::StudioFileKind::Module {Checkbox("Register module declaration", app.entry_registration())
                            .fontSize(12)}
                    if !draft.registration_error.is_empty() {Text(draft.registration_error)
                            .font(.caption)
                            .color(.orange)}
                    if draft.register {
                        if draft.targets.len()>1 {Text(strings.choose_the_target_parent_module)
                                .fontSize(12)
                                .weight(.semibold)}
                        for (index,target) in draft.targets.iter().enumerate() {
                            Button(format!("{}{}",if draft.target==Some(index) {"✓ "} else {""},target.label())) {app.new_entry.update(|draft|draft.target=Some(index));}
                            .fontSize(11)
                            .alignment(.leading)
                            .color(colors.accent_foreground)
                        }
                    }
                }
            }
            if !draft.module_note.is_empty() {
                Text(draft.module_note)
                    .font(.caption)
                    .color(colors.on_surface_variant)
                Checkbox("Update Rust module declarations",app.entry_module_updates())
                    .fontSize(12)
                Text(strings.updates_declarations_imports_and_symbol_references_are)
                    .font(.caption)
                    .color(colors.on_surface_variant)
            }
            if !draft.error.is_empty() { Text(draft.error)
                    .font(.caption)
                    .color(.red) }
            Layout {
                Layout {}
                .fill()
                Button(strings.cancel) { app.cancel_new_entry(); }
                .frame(88, 38)
                .background(colors.surface_container)
                .cornerRadius(8)
                Button(if draft.pending.is_some() { "Working…" } else if draft.source.is_some() { "Rename" } else { "Create" }) { app.submit_new_entry(); }
                .frame(100, 38)
                .background(colors.primary)
                .color(colors.on_primary)
                .cornerRadius(8)
            }
            .mode(.horizontal)
            .spacing(10)
        }
        .padding(24)
        .spacing(14)
        .frame(540, 470)
        .scrollable()
        .theme(settings.theme())
        .background(colors.surface_container_high)
        .color(colors.on_surface)
        .shortcut("enter", move || app.submit_new_entry())
        .task(move || app.clone().inspect_entry())
    }
}
