//! Source-backed controls for the selected Omen element.
use super::menu;
use crate::app::StudioEnvironment;
use omen::{AdaptiveDirection, Binding, LayoutMode, TextSnapshot, view};
use omen_macro_core::inspector::{
    ModifierDescriptor, Property, PropertyMode, adaptive_direction, is_color_modifier,
    layout_direction_property,
};
use std::path::PathBuf;

view! {
    pub CodeInspector {
        @Environment app: StudioEnvironment
        let colors=app.colors();
        let workspace=app.workspace;
        let active=workspace.active_file.clone();
        let editor=active.as_ref().map(|path|app.editor_document(path.clone()));
        let state=editor.as_ref().map(|document|document.state.get()).unwrap_or_default();
        let inspected=app.inspector;
        let query=(app.inspector_query).to_lowercase();
        let current=inspected.path==active && inspected.identity==state.text.identity() && inspected.cursor==state.selection.end;
        Layout {
            Layout {
                Icon("spanner")
                    .frame(15,15)
                    .color(colors.on_surface_variant)
                Text("Inspector")
                    .fontSize(12)
                    .weight(.semibold)
            }
            .mode(.horizontal)
            .spacing(6)
            .padding(8)
            .frame(.infinity, crate::theme::PANEL_HEADER_HEIGHT - 1.)
            Divider()
            Layout {
                if let Some(path)=active {
                    InspectorSelection(
                        path: path.clone(),
                        text: state.text.clone(),
                        cursor: state.selection.end
                    )
                    .key(format!("{}:{}:{}",path.display(),state.text.identity(),state.selection.end))
                    if current {
                        if !inspected.message.is_empty() { Text(inspected.message.clone())
                                .font(.caption)
                                .color(colors.on_surface_variant) }
                        if let Some(node)=inspected.node.clone() {
                            Layout {
                                Layout {
                                    Icon("file-omen-view").frame(19,19).color(colors.accent_foreground)
                                }
                                .frame(36,36).alignment(.center).cornerRadius(7).background(colors.selection)
                                Layout {
                                    Text(node.design_name().unwrap_or_else(|| node.component.clone())).fontSize(17).weight(.semibold).truncate()
                                    Text(node.path.join(" › ")).fontSize(11)
                                        .color(colors.on_surface_variant).truncate()
                                }.spacing(3).fill()
                            }
                            .mode(.horizontal).spacing(9).padding(9)
                            .frame(.infinity, 55)
                            .background(colors.surface).cornerRadius(8)
                            if node.name_property().is_none_or(|name| name.mode != PropertyMode::Expression) {
                                InspectorName(
                                    name: node.design_name().unwrap_or_default(),
                                    revision: inspected.revision
                                ).key(format!("{}:{}:name:{:?}", path.display(), node.name_range.start, node.design_name()))
                            }
                            if let Some(direction) = inspector_direction(&node) {
                                LayoutDirectionInspector(current: direction, revision: inspected.revision)
                            }
                            for category in ["Component","Layout","Sizing","Typography","Appearance","Scrolling","Code"] {
                                if node.properties.iter().any(|property|property.category==category && show_in_property_group(property)) {
                                    Layout {
                                        Text(category)
                                            .fontSize(12)
                                            .weight(.semibold)
                                            .color(colors.on_surface_variant)
                                        for (index,property) in node.properties.iter().filter(|property|property.category==category && show_in_property_group(property)).enumerate() {
                                            InspectorProperty(
                                                property: property.clone(),
                                                identity: inspected.revision
                                            )
                                            .key(format!("{}:{}:{}",node.name_range.start,category,index))
                                        }
                                    }
                                    .spacing(9).padding(9)
                                    .background(colors.surface).cornerRadius(8)
                                }
                            }
                            Divider()
                            Text("+ Modifier")
                                .fontSize(12)
                                .weight(.semibold)
                            Layout { TextField("Search modifiers")
                                    .value($app.inspector_query)
                                    .fontSize(12)
                                    .frame(.infinity,30)
                                    .background(colors.surface)
                                    .cornerRadius(4) }
                            .padding(1)
                            .background(colors.outline)
                            .cornerRadius(5)
                            for descriptor in node.available.iter().filter(|descriptor| !matches!(descriptor.name, "name" | "direction" | "mode") && (descriptor.label.to_lowercase().contains(&query) || descriptor.name.to_lowercase().contains(&query))) {
                                AddModifier(
                                    descriptor: descriptor.clone(),
                                    identity: inspected.revision
                                )
                            }
                            if !node.generated.is_empty() {
                                Divider()
                                Text("Generated modifiers")
                                    .fontSize(12)
                                    .weight(.semibold)
                                Text(node.generated.clone())
                                    .fontFamily("monospace")
                                    .fontSize(11)
                                    .color(colors.on_surface_variant)
                                Button("Copy modifiers") {}
                                .desktopAction(omen::DesktopAction::CopyText(node.generated))
                                .fontSize(11)
                                .color(colors.accent_foreground)
                            }
                        }
                    } else { Text("Reading component…")
                            .font(.caption)
                            .color(colors.on_surface_variant) }
                } else {
                    Text("Select a component")
                        .fontSize(17)
                        .weight(.semibold)
                    Text("Open an Omen source file and place the cursor on a component inside view!.")
                        .fontSize(12)
                        .color(colors.on_surface_variant)
                }
            }
            .padding(14)
            .spacing(12)
            .fill()
            .frame(.infinity,.infinity)
            .scrollable()
        }
        .spacing(0)
        .frame(.infinity,.infinity)
        .background(colors.surface_container)
        .color(colors.on_surface)
        .focusRequest($app.inspector_focus)
        .focusScope(true)
        .focusArrows(true)
        .accessibilityLabel("Code Inspector")
        .shortcut("escape",move || app.focus_editor())
        .shortcut(if cfg!(target_os="macos") {"cmd-i"} else {"ctrl-i"},move || app.focus_editor())
    }
}
fn show_in_property_group(property: &Property) -> bool {
    property.mode == PropertyMode::Expression
        || !(matches!(property.name.as_str(), "name" | "mode")
            || adaptive_direction(&property.value).is_some())
}

fn inspector_direction(node: &omen_macro_core::inspector::InspectedNode) -> Option<String> {
    if !matches!(node.component.as_str(), "Layout" | "AdaptiveLayout") {
        return None;
    }
    match layout_direction_property(node) {
        Some(property) if property.name == "adaptive" && property.value.is_empty() => {
            Some(adaptive_source(AdaptiveDirection::default()))
        }
        Some(property) if property.mode == PropertyMode::Expression => None,
        Some(property) => Some(property.value.clone()),
        None if node.component == "AdaptiveLayout" => {
            Some(adaptive_source(AdaptiveDirection::default()))
        }
        None => Some(".vertical".into()),
    }
}

fn direction_token(mode: LayoutMode) -> &'static str {
    match mode {
        LayoutMode::Vertical => ".vertical",
        LayoutMode::Horizontal => ".horizontal",
        LayoutMode::Stack => ".stack",
    }
}

fn adaptive_source(direction: AdaptiveDirection) -> String {
    let compact = direction_token(direction.compact);
    let expanded = direction_token(direction.expanded);
    let medium = direction.medium.map_or_else(String::new, |mode| {
        format!(", medium: {}", direction_token(mode))
    });
    format!(".adaptive(compact: {compact}{medium}, expanded: {expanded})")
}

view! {
    LayoutDirectionInspector(current: String, revision: u64) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let adaptive = adaptive_direction(&current);
        Layout {
            Text("Layout direction").fontSize(12).weight(.semibold)
            Layout {
                for (label, source) in [("Vertical", ".vertical"), ("Horizontal", ".horizontal"), ("Stack", ".stack"), ("Adaptive", ".adaptive(compact: .vertical, expanded: .horizontal)")] {
                    DirectionOption(
                        label: label,
                        source: source,
                        active: if label == "Adaptive" { adaptive.is_some() } else { current == source },
                        revision: revision
                    )
                }
            }.mode(.horizontal).spacing(3)
            if let Some(direction) = adaptive {
                for (label, compact) in [("Compact", true), ("Expanded", false)] {
                    Layout {
                        Text(label).fontSize(11).color(colors.on_surface_variant)
                        Layout {
                            for (label, mode) in [("Vertical", LayoutMode::Vertical), ("Horizontal", LayoutMode::Horizontal), ("Stack", LayoutMode::Stack)] {
                                DirectionOption(
                                    label: label,
                                    source: adaptive_source(if compact { AdaptiveDirection { compact: mode, ..direction } }
                                        else { AdaptiveDirection { expanded: mode, ..direction } }),
                                    active: mode == if compact { direction.compact } else { direction.expanded },
                                    revision: revision
                                )
                            }
                        }.mode(.horizontal).spacing(3)
                    }.spacing(4)
                }
            }
        }.spacing(8).padding(9).background(colors.surface).cornerRadius(8)
    }

    DirectionOption(label: String, source: String, active: bool, revision: u64) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Button(label) { if !active { app.inspector_layout_direction(&source, revision); } }
            .fontSize(11).padding(4).cornerRadius(4)
            .background(if active { colors.selection } else { colors.surface_container })
            .color(if active { colors.on_selection } else { colors.on_surface })
    }

}

view! {
    InspectorName(name: String, revision: u64) {
        @Environment app: StudioEnvironment
        @State draft = name.clone()
        let colors = app.colors();
        let input: Binding<String> = $draft.into();
        let field = input.clone();
        let save = omen::Action::new(move || {
            let value = input.get();
            if value != name {
                app.inspector_rename(&value, revision);
            }
        });
        Layout {
            Text("Name").fontSize(12).weight(.semibold)
            Layout {
                TextField("Name").value(field.clone()).frame(.infinity, 30).fill()
                    .fontSize(12).background(colors.surface_container)
                Button("Set name") { save.invoke(); }
                    .frame(64,30).cornerRadius(4).background(colors.selection).color(colors.on_selection)
            }.mode(.horizontal).spacing(6)
        }
        .padding(9).spacing(6).background(colors.surface).cornerRadius(8)
    }
}

view! {
    InspectorSelection(path: PathBuf, text: TextSnapshot, cursor: usize) {
        @Environment app: StudioEnvironment
        let analyze = app.inspector_task(path.clone(),text.clone(),cursor);
        Layout {}
        .frame(0,0)
        .task(analyze)
    }
}
fn display(value: &str, string: bool) -> String {
    if string {
        syn::parse_str::<syn::LitStr>(value)
            .map(|literal| literal.value())
            .unwrap_or_else(|_| value.to_string())
    } else {
        value.to_string()
    }
}
view! {
    InspectorProperty(property: Property, identity: u64) {
        @Environment app: StudioEnvironment
        @State context = property.clone()
        @State draft = display(&property.value,property.string || is_color_modifier(&property.name))
        @State replace = false
        let colors=app.colors();
        @State source_value = property.value.clone()
        let synced = sync_property($draft.into(),$source_value.into(), $context.into(), &property);
        let input = live_property(app.clone(), synced, $replace.into(), property.clone(), identity);
        let bound=property.mode==PropertyMode::Expression;
        Layout {
            Layout {
                Text(property.label.clone())
                    .fontSize(12)
                    .weight(.medium).fill()
                Text(if bound {"Bound"} else if property.mode==PropertyMode::Token {"Token"} else {"Literal"})
                    .fontSize(10)
                    .color(colors.on_surface_variant)
            }
            .mode(.horizontal)
            .spacing(6)
            if property.editable && !property.flag {
                if property.value == "true" || property.value == "false" {
                    Checkbox(property.label.clone(), boolean_draft(input.clone()))
                        .fontSize(12)
                } else {
                    if property.choices.is_empty() || matches!(property.name.as_str(), "spacing" | "padding") || is_color_modifier(&property.name) || bound {
                        Layout {
                            TextField(property.label.clone())
                                .value(input.clone())
                                .fontSize(12)
                                .frame(.infinity,30)
                                .background(colors.surface)
                                .cornerRadius(4)
                        }
                        .padding(1)
                        .background(colors.outline)
                        .cornerRadius(5)
                    }
                    if !property.choices.is_empty() && !property.string {
                        InspectorDropdown(
                            label: property.label.clone(),
                            choices: property.choices.clone(),
                            selection: input.clone()
                        )
                    }
                }
                if bound {
                    Checkbox("Replace binding with literal", $replace)
                        .fontSize(11)
                }

            } else {
                if property.flag {
                    Checkbox(property.label.clone(), enabled_modifier(app.clone(),context.key.clone(),identity))
                        .fontSize(12)
                } else {
                    Text(display(&property.value,property.string || is_color_modifier(&property.name)))
                        .fontFamily("monospace")
                        .fontSize(11)
                        .color(colors.on_surface_variant)
                }
            }
            Layout {
                if bound || !property.editable {
                    Button("Go to source") {app.inspector_reveal(&context.key);}
                    .fontSize(11)
                    .color(colors.accent_foreground)
                }
                if property.modifier_range.is_some() {
                    Button("Remove") {app.inspector_remove(&context.key,identity);}
                    .fontSize(11)
                    .color(colors.on_surface_variant)
                }
            }
            .mode(.horizontal)
            .spacing(10)
        }
        .padding(8)
        .spacing(7)
        .background(colors.surface)
        .cornerRadius(5)
    }
}
view! {
    AddModifier(descriptor: ModifierDescriptor, identity: u64) {
        @Environment app: StudioEnvironment
        let colors=app.colors();
        Button(format!("+ {} · {}",descriptor.label,descriptor.category)) {app.inspector_add(descriptor.name,identity);}
        .fontSize(11)
        .alignment(.leading)
        .padding(5)
        .color(colors.accent_foreground)
    }
}

fn boolean_draft(draft: Binding<String>) -> Binding<bool> {
    let read = draft.clone();
    Binding::new(
        move || read.get() == "true",
        move |checked| draft.set(checked.to_string()),
    )
}
fn enabled_modifier(app: StudioEnvironment, key: String, revision: u64) -> Binding<bool> {
    Binding::new(
        || true,
        move |enabled| {
            if !enabled {
                app.inspector_remove(&key, revision);
            }
        },
    )
}
view! {
    InspectorDropdown(label: String, choices: Vec<String>, selection: Binding<String>) {
        @Environment app: StudioEnvironment
        @State opened = false
        let colors = app.colors();
        let selected = selection.get();
        let close = $opened;
        let target = selection.clone();
        let options = choices.clone();
        Layout {
            Text(selected.trim_start_matches('.').to_string())
                .fontSize(12).fill()
            Icon("chevron-down")
                .frame(12,12)
                .color(colors.on_surface_variant)
        }
        .mode(.horizontal)
        .spacing(8)
        .padding(8)
        .frame(.infinity,32)
        .background(colors.surface_container_high)
        .cornerRadius(5)
        .onTap(move || opened = !opened)
        .accessibilityRole(.button)
        .accessibilityLabel(format!("{}: {}",label,selected))
        .accessibilityHint("Enter or Space opens values. Type to filter; arrows choose and Enter selects.")
        .popover($opened, move || menu::panel(view! { InspectorChoices(
    choices: options.clone(),
    selection: target.clone(),
    visible: close.clone()
) }))
    }
}
view! {
    InspectorChoices(choices: Vec<String>, selection: Binding<String>, visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        @State query = String::new()
        @State index = choices.iter().position(|choice| choice == &selection.get()).unwrap_or(0)
        let colors = app.colors();
        let current = selection.get();
        let filtered: Vec<_> = choices.iter().filter(|choice|choice.to_lowercase().contains(&query.to_lowercase())).cloned().collect();
        let count = filtered.len();
        let active = index.min(count.saturating_sub(1));
        let chosen = filtered.get(active).cloned();
        let target = selection.clone();
        let close = visible.clone();
        let dismiss = visible.clone();
        let toggle_close = visible.clone();
        Layout {
            TextField("Filter values…")
                .value($query)
                .autofocus(true)
                .onChange(move ||index=0)
                .fontSize(12)
                .frame(.infinity,30)
                .background(colors.surface)
                .cornerRadius(4)
            for (row,choice) in filtered.into_iter().enumerate().skip(active.saturating_sub(5)).take(6) {
                InspectorChoice(
                    choice: choice.clone(),
                    selected: current == choice,
                    highlighted: row == active,
                    selection: selection.clone(),
                    visible: visible.clone()
                )
            }
            if count == 0 {Text("No matching values")
                    .fontSize(12)
                    .color(colors.on_surface_variant)}
        }
        .padding(6)
        .spacing(2)
        .frame(240,260)
        .scrollable()
        .background(colors.surface_container)
        .color(colors.on_surface)
        .focusScope(true)
        .shortcut("down",move ||index=(index+1).min(count.saturating_sub(1)))
        .shortcut("up",move ||index=index.saturating_sub(1))
        .shortcut("home",move ||index=0)
        .shortcut("end",move ||index=count.saturating_sub(1))
        .shortcut("enter",move || {if let Some(choice)=chosen.clone() {target.set(choice);close.set(false);}})
        .shortcut("escape",move ||dismiss.set(false))
        .shortcut(if cfg!(target_os="macos") {"cmd-i"} else {"ctrl-i"},move ||{toggle_close.set(false);app.focus_editor();})
    }
}

fn live_property(
    app: StudioEnvironment,
    draft: Binding<String>,
    replace: Binding<bool>,
    property: Property,
    revision: u64,
) -> Binding<String> {
    let read = draft.clone();
    Binding::new(
        move || read.get(),
        move |value| {
            draft.set(value.clone());
            let source = if property.string {
                format!("{value:?}")
            } else {
                value.clone()
            };
            let inspected = app.inspector.get();
            if let Some(node) = inspected.node.as_ref() {
                let current = node.properties.iter().find(|item| item.key == property.key);
                if let Some(current) = current {
                    let text = inspected
                        .path
                        .as_ref()
                        .map(|path| {
                            app.editor_document(path.clone())
                                .state
                                .get()
                                .text
                                .to_string()
                        })
                        .unwrap_or_default();
                    if omen_macro_core::inspector::set_property(
                        &text,
                        node,
                        &current.key,
                        &source,
                        replace.get(),
                    )
                    .is_ok()
                    {
                        app.inspector_set(&current.key, &value, replace.get(), revision);
                    }
                }
            }
        },
    )
}
view! {
    InspectorChoice(choice: String, selected: bool, highlighted: bool, selection: Binding<String>, visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Button(format!("{}{}", choice.trim_start_matches('.'), if selected {"  ✓"} else {""})) {
            selection.set(choice.clone());
            visible.set(false);
        }
        .fontSize(12)
        .padding(7)
        .alignment(.leading)
        .frame(.infinity,30)
        .background(if highlighted {colors.selection} else {colors.surface_container})
        .color(if selected {colors.accent_foreground} else {colors.on_surface})
        .cornerRadius(4)
    }
}

fn sync_property(
    draft: Binding<String>,
    previous: Binding<String>,
    context: Binding<Property>,
    property: &Property,
) -> Binding<String> {
    if context.get().key != property.key {
        context.set(property.clone());
    }
    if previous.get() != property.value {
        draft.set(display(
            &property.value,
            property.string || is_color_modifier(&property.name),
        ));
        previous.set(property.value.clone());
    }
    draft
}
