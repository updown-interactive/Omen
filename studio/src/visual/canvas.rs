//! Design and frame controls mounted inside the existing document tab.
use super::view_tree::ViewTreeCache;
use super::{
    CanvasSettings, Component, ComponentEdit, EditorMode, Profile, VisualFrame, VisualTree, render,
};
use crate::{app::StudioEnvironment, views::menu};
use omen::{AnyView, Binding, Layout, TextSnapshot, View, ViewExt, ViewNode, view};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

view! {
    pub VisualToolbar(path: PathBuf, mode: EditorMode, breadcrumb: String) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            Layout {
                Icon("file-omen-view").frame(16,16).color(colors.accent_foreground)
                Text(breadcrumb.clone()).fontSize(12).weight(.medium).truncate()
            }
            .mode(.horizontal).spacing(8).fill()
            Layout {
                for (label, choice, mode_path) in [("Design", EditorMode::Design), ("Code", EditorMode::Code), ("Split", EditorMode::Split)].into_iter().map(|(label, mode)| (label, mode, path.clone())) {
                    Button(label) { app.set_document_mode(&mode_path, choice); }
                        .frame(57, 25)
                        .weight(if choice == mode { omen::Weight::Semibold } else { omen::Weight::Regular })
                        .background(if choice == mode { colors.selection } else { colors.surface_container_high })
                        .color(if choice == mode { colors.on_selection } else { colors.on_surface_variant })
                        .cornerRadius(5)
                }
            }
            .mode(.horizontal).spacing(2).padding(3).cornerRadius(8)
            .background(colors.surface_container_high)
        }
        .mode(.horizontal)
        .spacing(12)
        .padding(8)
        .fontSize(12)
        .frame(.infinity, 47)
        .background(colors.surface)
    }
}

view! {
    pub VisualCanvas(path: PathBuf, text: TextSnapshot, cursor: usize, size: [f32; 2]) {
        @Environment app: StudioEnvironment
        @State initial_cursor = cursor
        @State cursor_moved: Rc<std::cell::Cell<bool>> = Default::default()
        @State profiles = false
        @State measured = [0., 0.]
        @State restored = false
        let measurement = canvas_measurement(app.clone(), path.clone(), $measured.into(), $restored.into());
        let viewport = if measured[0] > 0. && measured[1] > 0. { measured } else { size };
        let canvas = app.canvas_settings(&path);
        let surface_focused = canvas
            .focused_frame
            .is_some_and(|id| canvas.frames.iter().any(|frame| frame.id == id));
        let colors = app.colors();
        let zoom = canvas.visible_zoom(viewport);
        let identity = text.identity();
        let result = (app.visual_result).filter(|result| result.path == path);
        let opening = !cursor_moved.get() && cursor == initial_cursor;
        let _tracked = cursor_moved.set(!opening);
        let declaration = if let Some(result) = result.as_ref() { result.selected(cursor, opening) } else { None };
        let marked = declaration.is_some_and(|declaration| declaration.marked);
        let preview = result.as_ref().zip(declaration)
            .and_then(|(result, declaration)| declaration.preview(result.preview_identity));
        let preview_error = result.as_ref().filter(|result| result.identity == identity)
            .and_then(|result| result.error.clone().or_else(|| declaration.and_then(|declaration| declaration.tree.as_ref().err().cloned())));
        let message = app.visual_message;
        let preview_status = if !message.is_empty() {
            message
        } else if result.as_ref().is_none_or(|result| result.identity != identity) {
            "Updating preview…".into()
        } else {
            preview_error.clone().or_else(|| preview.as_ref().map(|(_, tree)| tree.diagnostic.clone())).unwrap_or_default()
        };
        let task_path = path.clone();
        let show_profiles: Binding<bool> = $profiles.into();
        let dismiss_profiles = show_profiles.clone();
        let profile_path = path.clone();
        let undo_path = path.clone();
        let redo_path = path.clone();
        let interact_path = path.clone();
        let zoom_in_path = path.clone();
        let zoom_out_path = path.clone();
        let fit_path = path.clone();
        let center_path = path.clone();
        let undo_shortcut = path.clone();
        let redo_shortcut = path.clone();
        Layout {
            PreviewAnalysis(path: task_path, text: text.clone())
            .key(format!("{}:{identity}", path.display()))
            Layout {
                Button("+ Frame") { profiles = !profiles; }
                    .frame(76, 27).cornerRadius(6).weight(.semibold)
                    .background(colors.selection).color(colors.on_selection)
                    .popover(show_profiles, move || menu::panel(view! { ProfileMenu(path: profile_path.clone(), frame: None, visible: dismiss_profiles.clone()) }))
                Layout {
                    Button("↶") { app.visual_undo(&undo_path, false); }.frame(27, 25).accessibilityLabel("Undo")
                    Button("↷") { app.visual_undo(&redo_path, true); }.frame(27, 25).accessibilityLabel("Redo")
                }
                .mode(.horizontal).cornerRadius(6).background(colors.surface_container_high)
                Button(if canvas.interact { "Interacting" } else { "Interact" }) {
                    app.update_canvas(&interact_path, |canvas| canvas.interact = !canvas.interact);
                }
                .frame(80, 27).cornerRadius(6)
                .background(if canvas.interact { colors.selection } else { colors.surface_container_high })
                Text(preview_status.clone())
                    .fontSize(11).color(colors.on_surface_variant)
                    .truncate().fill()
                    .accessibilityHint(preview_status.clone())
                Layout {
                    Button("−") { app.update_canvas_motion(&zoom_out_path, |canvas| canvas.zoom_by(viewport, -10)); }
                        .frame(26,25).accessibilityLabel("Zoom out")
                    Text(format!("{zoom:.0}%")).fontFamily("monospace").fontSize(11).frame(40,18)
                    Button("+") { app.update_canvas_motion(&zoom_in_path, |canvas| canvas.zoom_by(viewport, 10)); }
                        .frame(26,25).accessibilityLabel("Zoom in")
                }
                .mode(.horizontal).cornerRadius(6).background(colors.surface_container_high)
                Button("Fit") { app.update_canvas(&fit_path, |canvas| canvas.fit(viewport)); }
                    .frame(34,27).cornerRadius(6).background(if canvas.fit_to_view { colors.selection } else { colors.surface_container }).hoverBackground(colors.surface_container_high)
                Button("Center") { app.update_canvas(&center_path, |canvas| canvas.center(viewport)); }
                    .frame(52,27).cornerRadius(6).hoverBackground(colors.surface_container_high)
            }
            .mode(.horizontal)
            .spacing(7)
            .padding(7)
            .fontSize(12)
            .scrollableHorizontal()
            .frame(.infinity, 42)
            .background(colors.surface_container)
            Divider()
            if let Some((revision, tree)) = preview {
                PreviewPane(tree: tree.clone(), path: path.clone(), identity: revision, cursor: cursor, marked: marked, canvas: canvas.clone(), viewport: viewport, measured: measured[0] > 0. && measured[1] > 0., measurement: measurement.clone())
            } else if let Some(error) = preview_error {
                Layout {
                    Text("Design preview unavailable").font(.headline)
                    Text(error.clone()).font(.caption)
                }.spacing(8).padding(20)
            } else {
                Text("Reading Omen view…").padding(16).color(colors.on_surface_variant)
            }
        }
        .spacing(0)
        .fill()
        .frame(.infinity, .infinity)
        .background(colors.surface_container)
        .shortcut(if !surface_focused { if cfg!(target_os = "macos") { "cmd-z" } else { "ctrl-z" } } else { "" }, move || if !surface_focused { app.visual_undo(&undo_shortcut, false); })
        .shortcut(if !surface_focused { if cfg!(target_os = "macos") { "cmd-shift-z" } else { "ctrl-shift-z" } } else { "" }, move || if !surface_focused { app.visual_undo(&redo_shortcut, true); })
    }
}

view! {
    PreviewAnalysis(path: PathBuf, text: TextSnapshot) {
        @Environment app: StudioEnvironment
        Layout {}
        .frame(0, 0)
        .task(move || app.analyze_visual(path.clone(), text.clone()))
    }
}

view! {
    PreviewPane(tree: Arc<VisualTree>, path: PathBuf, identity: u64, cursor: usize, marked: bool, canvas: CanvasSettings, viewport: [f32; 2], measured: bool, measurement: Binding<[f32; 2]>) {
        @Environment app: StudioEnvironment
        @State view_tree_cache: ViewTreeCache = Default::default()
        @State library = false
        @State insertion: Option<omen::FlowPlacement> = None
        let colors = app.colors();
        let selected = tree.selected(cursor);
        let target = selected.unwrap_or(0);
        let library_visible: Binding<bool> = $library.into();
        let insertion_position: Binding<Option<omen::FlowPlacement>> = $insertion.into();
        let position = insertion.filter(|_| parent_is_flow(&tree, target));
        let focused_frame = canvas
            .focused_frame
            .filter(|id| canvas.frames.iter().any(|frame| frame.id == *id));
        let surface_focused = focused_frame.is_some();
        let design = !canvas.interact && !surface_focused;
        let guide = if library && design { render::InsertionGuide::new(&tree, target, position) } else { None };
        let active_frame = canvas.frames.iter().find(|frame| frame.id == canvas.active_frame).cloned();
        let active_width = active_frame.as_ref().map(|frame| f32::from(frame.width));
        let layout_label = guide.zip(active_width).map(|(guide, width)| guide.label(&tree, width));
        let width = active_width.unwrap_or(0.);
        let scale = canvas.visible_zoom(viewport) / 100.;
        let origin = canvas.origin(viewport, scale);
        let frames: Vec<_> = canvas.frames.iter().filter(|frame| !measured || canvas.interact || surface_focused || frame.is_visible(viewport, scale, origin)).cloned().collect();
        let select = app.visual_selector(path.clone(), identity, tree.clone());
        let floating_tree = if design && viewport[0] >= 120. && viewport[1] >= 54.
            && let Some(frame) = active_frame.as_ref()
        {
            Some(view_tree_cache.view(&tree, frame, selected, &select, viewport))
        } else { None };
        let grid = (app.settings).design.show_grid.then_some(colors.grid);
        let navigation = app.canvas_navigation(path.clone(), viewport);
        let reorder = reorder_sink(&app, &path, identity, &tree);
        let offscreen = !canvas.frames.is_empty() && !canvas.frames.iter().any(|frame| frame.is_visible(viewport, scale, origin));
        let recovery_path = path.clone();
        let recovery_app = app.clone();
        let frame_app = app.clone();
        let previous = target.checked_sub(1);
        let next = (target + 1 < tree.nodes.len()).then_some(target + 1);
        let parent = tree.parent(target);
        let child = tree.children(Some(target)).next();
        let select_previous = select.clone();
        let select_next = select.clone();
        let select_parent = select.clone();
        let select_child = select.clone();
        let earlier = VisualEdit::new(&app, &path, identity, &tree, target);
        let later = earlier.clone();
        let delete = earlier.clone();
        let can_delete = design && selected.is_some() && parent.is_some();
        Layout {
            if canvas.interact {
                Text("Interactive simulation · project actions unavailable").font(.caption).padding(8)
            } else if surface_focused {
                Text("Frame focused · design surface controls paused").font(.caption).padding(8)
            } else {
                Composition(path: path.clone(), identity: identity, tree: tree.clone(), target: target, width: width, compact: viewport[0] < 475., visible: library_visible.clone(), insertion: insertion_position.clone(), layout_label: layout_label.clone())
            }
            if canvas.frames.is_empty() {
                Text("Add a frame to preview this view.").padding(18)
            } else {
                PreviewSurface(navigation: (!surface_focused).then_some(navigation.clone()), measurement: measurement.clone(), background: colors.canvas, grid: grid, grid_transform: [origin[0], origin[1], scale], content: view! {
                    Layout {
                        if offscreen {
                            Layout {
                                Button("Bring frames into view") {
                                    recovery_app.update_canvas(&recovery_path, |canvas| canvas.recover_view(viewport));
                                }
                                .padding(10).cornerRadius(6)
                                .background(colors.selection).color(colors.on_selection)
                            }
                            .frame(.infinity, .infinity).alignment(.center)
                        }
                        for frame in frames {
                            PositionedFrame(
                                position: position_frame(&frame, scale, origin),
                                drag: design.then(|| omen::PreviewFrameDrag {
                                    select: frame_selection(frame_app.clone(), path.clone(), frame.id),
                                    position: frame_app.frame_position(path.clone(), frame.id, frame.position, viewport),
                                    scale,
                                }),
                                scale: scale,
                                content: AnyView::new(FrameCard {
                                    frame: frame.clone(),
                                    active: frame.id == canvas.active_frame,
                                    focused: focused_frame == Some(frame.id),
                                    surface_focused,
                                    path: path.clone(),
                                    tree: tree.clone(),
                                    identity,
                                    marked,
                                    selected,
                                    select: select.clone(),
                                    interact: canvas.interact,
                                    scale,
                                    viewport,
                                    reorder: reorder.clone(),
                                    guide: if frame.id == canvas.active_frame { guide } else { None },
                                })
                            ).key(frame.id.to_string())
                        }
                        if let Some(panel) = floating_tree.clone() {
                            TreePanel(content: panel)
                        }
                    }
                    .frame(.infinity, .infinity)
                })
                .fill()
                .frame(.infinity, .infinity)
                .shortcut(if can_delete { "backspace" } else { "" }, move || { if can_delete { delete.apply(ComponentEdit::Delete); } })
            }
        }
        .spacing(0)
        .fill()
        .frame(.infinity, .infinity)
        .shortcut(if design { "alt-up" } else { "" }, move || { if design && let Some(index) = previous { select_previous(index); } })
        .shortcut(if design { "alt-down" } else { "" }, move || { if design && let Some(index) = next { select_next(index); } })
        .shortcut(if design { "alt-left" } else { "" }, move || { if design && let Some(index) = parent { select_parent(index); } })
        .shortcut(if design { "alt-right" } else { "" }, move || { if design && let Some(index) = child { select_child(index); } })
        .shortcut(if design { "alt-shift-up" } else { "" }, move || { if design && parent.is_some() { earlier.apply(ComponentEdit::Move(true)); } })
        .shortcut(if design { "alt-shift-down" } else { "" }, move || { if design && parent.is_some() { later.apply(ComponentEdit::Move(false)); } })
    }
}

struct TreePanel {
    content: AnyView,
}

impl View for TreePanel {
    fn into_node(self) -> ViewNode {
        self.content.into_node()
    }
}

view! {
    Composition(path: PathBuf, identity: u64, tree: Arc<VisualTree>, target: usize, width: f32, compact: bool, visible: Binding<bool>, insertion: Binding<Option<omen::FlowPlacement>>, layout_label: Option<String>) {
        @Environment app: StudioEnvironment
        @State navigation = false
        @State editing = false
        let colors = app.colors();
        let toggle = visible.clone();
        let close_for_navigation = visible.clone();
        let close_for_edit = visible.clone();
        let reset = insertion.clone();
        let close = visible.clone();
        let nav_visible: Binding<bool> = $navigation.into();
        let edit_visible: Binding<bool> = $editing.into();
        let dismiss_nav = nav_visible.clone();
        let dismiss_edit = edit_visible.clone();
        let select = app.visual_selector(path.clone(), identity, tree.clone());
        let label = |i: usize| tree.nodes[i].design_name().unwrap_or_else(|| render::component_label(&tree.nodes[i], width));
        let selected = tree.nodes.get(target).map(|_| label(target)).unwrap_or_default();
        let parent = tree.parent(target).map(|i| (i, label(i)));
        let child = tree.children(Some(target)).next().map(|i| (i, label(i)));
        let navigable = parent.is_some() || child.is_some();
        let menu_path = path.clone();
        let menu_tree = tree.clone();
        let edit = VisualEdit::new(&app, &path, identity, &tree, target);
        Layout {
            Layout {
                Text("Selected").fontSize(11).color(colors.on_surface_variant)
                if navigable {
                    Button(format!("{selected}  ▾")) { editing = false; close_for_navigation.set(false); navigation = !navigation; }
                        .padding(5).cornerRadius(5).truncate().weight(.semibold)
                        .background(colors.selection).color(colors.on_selection)
                        .accessibilityLabel(format!("Change selection, currently {selected}"))
                        .popover(nav_visible, move || navigation_menu(parent.clone(), child.clone(), select.clone(), dismiss_nav.clone(), colors))
                } else {
                    Text(selected.clone()).weight(.semibold).color(colors.on_surface).truncate()
                }
            }.mode(.horizontal).spacing(6)
            Layout {
                Button("+ Add component") { navigation = false; editing = false; reset.set(None); toggle.set(!toggle.get()); }
                    .frame(116,28).cornerRadius(6).weight(.semibold)
                    .background(colors.primary).color(colors.on_primary)
                    .popover(visible, move || menu::panel(view! {
                        ComponentMenu(path: menu_path.clone(), identity: identity, tree: menu_tree.clone(), target: target, visible: close.clone(), insertion: insertion.clone(), layout_label: layout_label.clone())
                    }))
                if tree.parent(target).is_some() {
                    Button("Edit selection  ▾") { navigation = false; close_for_edit.set(false); editing = !editing; }
                        .frame(110,28).cornerRadius(6)
                        .background(colors.surface_container_high).hoverBackground(colors.selection)
                        .popover(edit_visible, move || edit_menu(&edit, width, &dismiss_edit, colors))
                }
            }.mode(.horizontal).spacing(6)
        }
        .mode(if compact { omen::LayoutMode::Vertical } else { omen::LayoutMode::Horizontal })
        .alignment(.topLeading)
        .spacing(if compact { 4 } else { 18 })
        .padding(6)
        .scrollableHorizontal()
        .fontSize(12)
        .frame(.infinity, if compact { 70 } else { 42 })
        .background(colors.surface_container)
    }
}

fn menu_command(
    label: String,
    action: impl Fn() + 'static,
    colors: crate::theme::StudioColors,
) -> AnyView {
    omen::Button::new(label, action)
        .alignment(omen::Alignment::Leading)
        .frame(f64::INFINITY, 30.)
        .padding(6.)
        .corner_radius(5.)
        .truncate()
        .hover_background(colors.selection)
}

fn menu_heading(label: &str, colors: crate::theme::StudioColors) -> AnyView {
    omen::Text::new(label)
        .font_size(11.)
        .weight(omen::Weight::Semibold)
        .color(colors.on_surface_variant)
        .frame(f64::INFINITY, 22.)
        .alignment(omen::Alignment::Leading)
}

fn navigation_menu(
    parent: Option<(usize, String)>,
    child: Option<(usize, String)>,
    select: render::Select,
    visible: Binding<bool>,
    colors: crate::theme::StudioColors,
) -> AnyView {
    let mut panel = Layout::new();
    let mut height = 16.;
    for (relation, item) in [("Select parent", parent), ("Select first child", child)] {
        if let Some((index, label)) = item {
            let select = select.clone();
            let close = visible.clone();
            panel = panel
                .child(menu_heading(relation, colors))
                .child(menu_command(
                    label,
                    move || {
                        close.set(false);
                        select(index);
                    },
                    colors,
                ));
            height += 52.;
        }
    }
    menu::panel(
        panel
            .padding(8.)
            .frame(228., height)
            .background(colors.surface_container_high)
            .corner_radius(8.),
    )
}

fn edit_menu(
    edit: &VisualEdit,
    width: f32,
    visible: &Binding<bool>,
    colors: crate::theme::StudioColors,
) -> AnyView {
    let tree = &edit.tree;
    let target = edit.target;
    let command = |label: String, operation| {
        let edit = edit.clone();
        let close = visible.clone();
        menu_command(
            label,
            move || {
                close.set(false);
                edit.apply(operation);
            },
            colors,
        )
    };
    let mut placement = Vec::new();
    let parent = tree.parent(target);
    if let Some(parent) = parent {
        let siblings: Vec<_> = tree.children(Some(parent)).collect();
        if let Some(index) = siblings.iter().position(|&i| i == target) {
            for (neighbor, earlier) in [(index.checked_sub(1), true), (Some(index + 1), false)] {
                if let Some(node) = neighbor
                    .and_then(|i| siblings.get(i))
                    .map(|&i| &tree.nodes[i])
                {
                    let name = node
                        .design_name()
                        .unwrap_or_else(|| render::component_label(node, width));
                    let direction = if earlier { "before" } else { "after" };
                    placement.push(
                        command(
                            format!("Move {direction} {name}"),
                            ComponentEdit::Move(earlier),
                        )
                        .into_node(),
                    );
                }
            }
        }
        if tree.nodes[parent].component == "Layout" && tree.nodes[parent].children_end.is_some() {
            let pinned = tree.nodes[target]
                .properties
                .iter()
                .any(|p| p.name == "pin");
            let label = if pinned {
                "Return to layout flow"
            } else {
                "Pin to position inside parent"
            };
            placement.push(command(label.into(), ComponentEdit::TogglePin).into_node());
        }
    }
    let mut panel = Layout::new();
    let mut height = 16.;
    if !placement.is_empty() {
        height += 22. + 30. * placement.len() as f32;
        panel = panel
            .child(menu_heading("Placement", colors))
            .children(placement);
    }
    panel = panel.child(menu_heading("Structure", colors));
    height += 22.;
    if !tree.nodes[target]
        .properties
        .iter()
        .any(|p| p.name == "key" && p.modifier_range.is_some())
    {
        panel = panel.child(command(
            "Duplicate component".into(),
            ComponentEdit::Duplicate,
        ));
        height += 30.;
    }
    if parent_is_flow(tree, target) {
        panel = panel.child(command("Wrap in a new layout".into(), ComponentEdit::Wrap));
        height += 30.;
    }
    panel = panel
        .child(omen::Divider::new())
        .child(command("Delete component".into(), ComponentEdit::Delete).color(omen::Color::Red));
    menu::panel(
        panel
            .padding(8.)
            .frame(250., height + 31.)
            .background(colors.surface_container_high)
            .corner_radius(8.),
    )
}

view! {
    ComponentMenu(path: PathBuf, identity: u64, tree: Arc<VisualTree>, target: usize, visible: Binding<bool>, insertion: Binding<Option<omen::FlowPlacement>>, layout_label: Option<String>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        let edit = VisualEdit::new(&app, &path, identity, &tree, target);
        let sibling = parent_is_flow(&tree, target);
        let position = insertion.get().filter(|_| sibling);
        let before = insertion.clone();
        let after = insertion.clone();
        let append = insertion.clone();
        let frame = app.canvas_settings(&path).active_frame;
        let scope = format!("{}:{frame}", placement_scope(&path, identity, &tree));
        let has_guide = layout_label.is_some();
        Layout {
            if let Some(label) = layout_label.clone() {
                Text(label).fontSize(11).weight(.medium).color(colors.accent_foreground)
                    .padding(5).truncate()
            }
            if sibling {
                Text("Insert relative to selection").fontSize(11).weight(.semibold).color(colors.on_surface_variant)
                Layout {
                    Button("Before") { before.set(Some(omen::FlowPlacement::Before)); }
                        .padding(5).background(if position == Some(omen::FlowPlacement::Before) { colors.selection } else { colors.surface_container_high })
                    Button("After") { after.set(Some(omen::FlowPlacement::After)); }
                        .padding(5).background(if position == Some(omen::FlowPlacement::After) { colors.selection } else { colors.surface_container_high })
                    Button(if tree.flow_container(target) { "Inside" } else { "At end" }) { append.set(None); }
                        .padding(5).background(if position.is_none() { colors.selection } else { colors.surface_container_high })
                }.mode(.horizontal).spacing(4)
            }
            for component in Component::ALL {
                ComponentChoice(
                    component: component, edit: edit.clone(), visible: visible.clone(),
                    position: position, scope: scope.clone()
                )
            }
        }
        .padding(8)
        .frame(206, 26. + Component::ALL.len() as f32 * 30. + if sibling { 47. } else { 0. } + if has_guide { 24. } else { 0. })
        .cornerRadius(9)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

fn parent_is_flow(tree: &VisualTree, target: usize) -> bool {
    tree.parent(target)
        .is_some_and(|parent| tree.flow_container(parent))
}

struct ComponentChoice {
    component: Component,
    edit: VisualEdit,
    visible: Binding<bool>,
    position: Option<omen::FlowPlacement>,
    scope: String,
}

impl View for ComponentChoice {
    fn into_node(self) -> ViewNode {
        let Self {
            component,
            edit,
            visible,
            position,
            scope,
        } = self;
        let dismiss = visible.clone();
        let click_edit = edit.clone();
        let mut node = omen::Button::new(component.label(), move || {
            visible.set(false);
            click_edit.apply(
                position.map_or(ComponentEdit::Insert(component), |placement| {
                    ComponentEdit::InsertAt {
                        target: click_edit.target,
                        placement,
                        component,
                    }
                }),
            );
        })
        .alignment(omen::Alignment::Leading)
        .frame(f64::INFINITY, 30.)
        .padding(7.)
        .corner_radius(5.)
        .hover_background(omen::Color::Surface)
        .accessibility_hint("Click to insert or drag into the active preview frame")
        .into_node();
        node.style.preview_mut().insert = Some(omen::PreviewInsert {
            scope,
            label: component.label().into(),
            begin: omen::Action::new(move || dismiss.set(false)),
            insert: Binding::new(
                || (0, omen::FlowPlacement::Into),
                move |(offset, placement)| {
                    if let Some(target) = edit
                        .tree
                        .nodes
                        .iter()
                        .position(|node| node.name_range.start == offset)
                    {
                        edit.apply(ComponentEdit::InsertAt {
                            target,
                            placement,
                            component,
                        });
                    }
                },
            ),
        });
        node
    }
}

view! {
    ProfileMenu(path: PathBuf, frame: Option<u64>, visible: Binding<bool>) {
        @Environment app: StudioEnvironment
        let colors = app.colors();
        Layout {
            for (profile, profile_path, close) in Profile::ALL.into_iter().map(|profile| (profile, path.clone(), visible.clone())) {
                Button(profile.label()) {
                    close.set(false);
                    app.update_canvas(&profile_path, |canvas| {
                        if let Some(id) = frame {
                            if let Some(frame) = canvas.frames.iter_mut().find(|frame| frame.id == id) {
                                let (width, height) = profile.size();
                                if frame.name == frame.profile.label() {
                                    frame.name = profile.label().into();
                                }
                                frame.profile = profile;
                                frame.width = width;
                                frame.height = height;
                            }
                        } else {
                            canvas.add(profile, None);
                        }
                    });
                }.alignment(.leading).frame(.infinity, 29).padding(7).cornerRadius(5).hoverBackground(colors.selection)
            }
        }
        .padding(8)
        .frame(200, 335)
        .cornerRadius(9)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

fn canvas_measurement(
    app: StudioEnvironment,
    path: PathBuf,
    measured: Binding<[f32; 2]>,
    restored: Binding<bool>,
) -> Binding<[f32; 2]> {
    let read = measured.clone();
    Binding::new(
        move || read.get(),
        move |viewport| {
            measured.set(viewport);
            if viewport[0] > 0. && viewport[1] > 0. && !restored.get() {
                restored.set(true);
                app.restore_canvas_view(&path, viewport);
            }
        },
    )
}

struct PreviewSurface {
    navigation: Option<Binding<omen::PreviewGesture>>,
    measurement: Binding<[f32; 2]>,
    background: omen::Color,
    grid: Option<omen::Color>,
    grid_transform: [f32; 3],
    content: AnyView,
}

impl View for PreviewSurface {
    fn into_node(self) -> ViewNode {
        let mut node = Layout::new()
            .child(self.content)
            .background(self.background)
            .into_node();
        let preview = node.style.preview_mut();
        preview.navigation = self.navigation;
        preview.grid = self.grid;
        preview.grid_transform = Some(self.grid_transform);
        preview.viewport_size = Some(self.measurement);
        node
    }
}

fn position_frame(frame: &VisualFrame, scale: f32, origin: [f32; 2]) -> [f32; 2] {
    std::array::from_fn(|axis| frame.position[axis] as f32 * scale + origin[axis])
}

struct PositionedFrame {
    drag: Option<omen::PreviewFrameDrag>,
    position: [f32; 2],
    scale: f32,
    content: AnyView,
}

impl View for PositionedFrame {
    fn into_node(self) -> ViewNode {
        let mut content = self.content.into_node();
        content.style.preview_mut().scale = Some(self.scale);
        let mut node = Layout::new().child(content).into_node();
        node.style.preview_mut().position = Some(self.position);
        node.style.preview_mut().frame_drag = self.drag;
        node
    }
}

fn frame_selection(app: StudioEnvironment, path: PathBuf, id: u64) -> omen::Action {
    omen::Action::new(move || app.update_canvas_motion(&path, |canvas| canvas.active_frame = id))
}

struct FrameGrip {
    drag: Option<omen::PreviewFrameDrag>,
    content: AnyView,
}

impl View for FrameGrip {
    fn into_node(self) -> ViewNode {
        let hint = if self.drag.is_some() {
            "Drag to move frame"
        } else {
            "Frame drag disabled"
        };
        let mut node = Layout::new()
            .child(omen::Text::new("⠿"))
            .child(self.content)
            .mode(omen::LayoutMode::Horizontal)
            .spacing(6)
            .frame(f64::INFINITY, 20.)
            .accessibility_hint(hint)
            .into_node();
        node.style.preview_mut().frame_drag = self.drag;
        node
    }
}

struct DeviceShell {
    content: Rc<AnyView>,
    frame: VisualFrame,
}

impl View for DeviceShell {
    fn into_node(self) -> ViewNode {
        let shell = self.frame.profile.shell();
        let [side, top, bottom] = shell.insets();
        let mut node = Layout::new()
            .child(self.content.as_ref().clone())
            .frame(
                f32::from(self.frame.width) + side * 2.,
                f32::from(self.frame.height) + top + bottom,
            )
            .into_node();
        node.style.preview_mut().shell = Some((shell, self.frame.dark));
        node
    }
}

#[derive(Clone, PartialEq)]
struct FrameRenderKey {
    identity: u64,
    profile: Profile,
    width: u16,
    height: u16,
    dark: bool,
    safe_area: bool,
    locale: String,
    selected: Option<usize>,
    interact: bool,
    inert: bool,
    guide: Option<render::InsertionGuide>,
}

type FrameRenderCache = Rc<RefCell<Option<(FrameRenderKey, Arc<VisualTree>, Rc<AnyView>)>>>;

fn cached_frame_content(
    cache: &FrameRenderCache,
    frame: &VisualFrame,
    identity: u64,
    selected: Option<usize>,
    interact: bool,
    inert: bool,
    guide: Option<render::InsertionGuide>,
    tree: &Arc<VisualTree>,
    build: impl FnOnce(bool) -> AnyView,
) -> Rc<AnyView> {
    let key = FrameRenderKey {
        identity,
        profile: frame.profile,
        width: frame.width,
        height: frame.height,
        dark: frame.dark,
        safe_area: frame.safe_area,
        locale: frame.locale.clone(),
        selected,
        interact,
        inert,
        guide,
    };
    let mut cache = cache.borrow_mut();
    if let Some((cached_key, previous, content)) = &*cache
        && *cached_key == key
        && Arc::ptr_eq(previous, tree)
    {
        return content.clone();
    }
    let structure_changed = cache.as_ref().is_some_and(|(_, previous, _)| {
        !Arc::ptr_eq(previous, tree) && !previous.same_structure(tree)
    });
    let content = Rc::new(build(structure_changed));
    *cache = Some((key, tree.clone(), content.clone()));
    content
}

view! {
    FrameCard(frame: VisualFrame, active: bool, focused: bool, surface_focused: bool, path: PathBuf, tree: Arc<VisualTree>, identity: u64, marked: bool, selected: Option<usize>, select: render::Select, interact: bool, scale: f32, viewport: [f32; 2], reorder: render::Reorder, guide: Option<render::InsertionGuide>) {
        @Environment app: StudioEnvironment
        @State options = false
        @State interaction: render::InteractivePreview = Default::default()
        @State render_cache: FrameRenderCache = Default::default()
        let id = frame.id;
        let select_frame = frame_selection(app.clone(), path.clone(), id);
        let select_node = select.clone();
        let activate_frame = select_frame.clone();
        let select_component: render::Select = Rc::new(move |index| {
            select_node(index);
            activate_frame.invoke();
        });
        let frame_interactive = interact || focused;
        let inert = surface_focused && !focused;
        let can_drag = !interact && !surface_focused;
        let preview_session = interaction;
        let cache = render_cache;
        let content = cached_frame_content(&cache, &frame, identity, selected, frame_interactive, inert, guide, &tree, |structure_changed| {
            if structure_changed { preview_session.reset(); }
            let mode = if frame_interactive {
                render::FrameInteraction::Interactive(&preview_session)
            } else if inert {
                render::FrameInteraction::Inert
            } else {
                render::FrameInteraction::Design
            };
            render::frame_content(&tree, &frame, selected, select_component, mode, &reorder, guide)
        });
        let colors = app.colors();
        let visible: Binding<bool> = $options.into();
        let focus_app = app.clone();
        let focus_path = path.clone();
        let drag_path = path.clone();
        let drag = can_drag.then(|| omen::PreviewFrameDrag {
            select: select_frame,
            position: app.frame_position(drag_path, id, frame.position, viewport),
            scale,
        });
        let caption = if marked {
            format!("@Design · {} · {} × {}", tree.declaration_name().unwrap_or_default(), frame.width, frame.height)
        } else {
            format!("{} × {} · GPUI simulation", frame.width, frame.height)
        };
        let width = frame.card_width();
        let height = frame.card_height();
        let settings_frame = frame.clone();
        let key = format!("{}:{id}:{}:{}:{}:{}", frame.name, frame.width, frame.height, frame.locale, frame.profile.label());
        Layout {
            Layout {
                Layout {
                    FrameGrip(drag: drag, content: view! {
                        Text(frame.name.clone())
                            .weight(if active || focused { omen::Weight::Semibold } else { omen::Weight::Regular })
                            .color(if active || focused { colors.accent_foreground } else { colors.on_surface }).truncate().fill()
                    }).fill()
                    Button("⋯") { options = !options; }
                        .frame(23,20).cornerRadius(4).hoverBackground(colors.surface_container)
                        .accessibilityLabel("Frame settings")
                        .popover(visible, move || menu::panel(view! { FrameSettings(path: path.clone(), frame: settings_frame.clone()).key(key.clone()) }))
                    Layout {
                        Icon(if focused { "eye" } else { "eye-closed" })
                            .frame(14, 14)
                    }
                    .frame(23, 20)
                    .alignment(.center)
                    .cornerRadius(4)
                    .background(if focused { colors.selection } else { colors.canvas })
                    .color(if focused { colors.on_selection } else { colors.on_surface_variant })
                    .hoverBackground(if focused { colors.selection } else { colors.surface_container })
                    .onTap(move || {
                        focus_app.update_canvas(&focus_path, |canvas| {
                            canvas.focused_frame = if canvas.focused_frame == Some(id) {
                                None
                            } else {
                                canvas.active_frame = id;
                                Some(id)
                            };
                        });
                    })
                    .accessibilityRole(.button)
                    .accessibilityLabel(if focused { "Unfocus frame" } else { "Focus frame" })
                    .accessibilityHint(if focused {
                        "Return to design surface controls"
                    } else {
                        "Focus mouse and scroll controls on this frame"
                    })
                }
                .mode(.horizontal)
                .spacing(5)
                .fontSize(12)
                .frame(.infinity, 20)
                Text(caption.clone())
                    .truncate()
                    .fontSize(10)
                    .fontFamily("monospace")
                    .color(colors.on_surface_variant)

            }
            .spacing(2)
            .alignment(.topLeading)
            .frame(width, VisualFrame::LABEL_HEIGHT)
            DeviceShell(content: content.clone(), frame: frame.clone())
        }
        .spacing(0)
        .alignment(.topLeading)
        .frame(width, height)
    }
}

view! {
    FrameSettings(path: PathBuf, frame: VisualFrame) {
        @Environment app: StudioEnvironment
        @State profiles = false
        @State name = frame.name.clone()
        @State width = frame.width.to_string()
        @State height = frame.height.to_string()
        @State locale = frame.locale.clone()
        let colors = app.colors();
        let visible: Binding<bool> = $profiles.into();
        let close = visible.clone();
        let id = frame.id;
        let profile_path = path.clone();
        let details_path = path.clone();
        let rotate_path = path.clone();
        let appearance_path = path.clone();
        let safe_path = path.clone();
        let left_path = path.clone();
        let right_path = path.clone();
        let up_path = path.clone();
        let down_path = path.clone();
        let duplicate_path = path.clone();
        let reset_path = path.clone();
        Layout {
            Text("Frame settings").fontSize(15).weight(.semibold).frame(.infinity, 20)
            Button(frame.profile.label()) { profiles = !profiles; }
                .frame(.infinity,28).alignment(.leading).padding(6).cornerRadius(5)
                .background(colors.surface)
                .popover(visible, move || menu::panel(view! { ProfileMenu(path: profile_path.clone(), frame: Some(id), visible: close.clone()) }))
            TextField("Frame name").value($name).frame(.infinity, 28).background(colors.surface)
            Text("Viewport · logical px").fontSize(11).color(colors.on_surface_variant)
            Layout {
                TextField("Width").value($width).frame(128, 28).background(colors.surface)
                TextField("Height").value($height).frame(128, 28).background(colors.surface)
            }.mode(.horizontal).spacing(8)
            TextField("Locale").value($locale).frame(.infinity, 28).background(colors.surface)
            Button("Apply frame details") {
                let width_draft = width;
                let height_draft = height;
                let name_draft = name;
                let locale_draft = locale;
                match (width_draft.trim().parse::<u16>(), height_draft.trim().parse::<u16>()) {
                    (Ok(w), Ok(h)) if (120..=4096).contains(&w) && (120..=4096).contains(&h) => {
                        let name = name_draft.trim().chars().take(80).collect::<String>();
                        let locale = locale_draft.trim().chars().take(32).collect::<String>();
                        update_frame(&app, &details_path, id, |frame| {
                            if !name.is_empty() { frame.name = name; }
                            frame.locale = if locale.is_empty() { "en".into() } else { locale };
                            frame.width = w;
                            frame.height = h;
                        });
                    }
                    _ => app.visual_message = "Frame dimensions must be between 120 and 4096.".into(),
                }
            }
                .frame(.infinity,26).alignment(.leading).padding(6).cornerRadius(5).background(colors.selection).color(colors.on_selection)
            Divider()
            Button("Rotate") { update_frame(&app, &rotate_path, id, |frame| std::mem::swap(&mut frame.width, &mut frame.height)); }
                .frame(.infinity,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection)
            Button(if frame.dark { "Switch to light appearance" } else { "Switch to dark appearance" }) {
                update_frame(&app, &appearance_path, id, |frame| frame.dark = !frame.dark);
            }.frame(.infinity,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection)
            Button(if frame.safe_area { "Hide safe area" } else { "Show safe area" }) {
                update_frame(&app, &safe_path, id, |frame| frame.safe_area = !frame.safe_area);
            }.frame(.infinity,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection)
            Layout {
                Text("Move frame").padding(6).fill()
                Button("←") { move_frame(&app, &left_path, id, [-48, 0]); }
                    .frame(28,26).cornerRadius(4).hoverBackground(colors.selection).accessibilityLabel("Move frame left")
                Button("↑") { move_frame(&app, &up_path, id, [0, -48]); }
                    .frame(28,26).cornerRadius(4).hoverBackground(colors.selection).accessibilityLabel("Move frame up")
                Button("↓") { move_frame(&app, &down_path, id, [0, 48]); }
                    .frame(28,26).cornerRadius(4).hoverBackground(colors.selection).accessibilityLabel("Move frame down")
                Button("→") { move_frame(&app, &right_path, id, [48, 0]); }
                    .frame(28,26).cornerRadius(4).hoverBackground(colors.selection).accessibilityLabel("Move frame right")
            }.mode(.horizontal).spacing(4).frame(.infinity,26)
            Divider()
            Layout {
                Button("Duplicate frame") {
                    app.update_canvas(&duplicate_path, |canvas| {
                        if let Some(frame) = canvas.frames.iter().find(|frame| frame.id == id).cloned() {
                            canvas.add(frame.profile, Some(frame));
                        }
                    });
                }.frame(128,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection)
                Button("Reset frame") {
                    update_frame(&app, &reset_path, id, |frame| {
                        let position = frame.position;
                        *frame = VisualFrame::new(id, frame.profile);
                        frame.position = position;
                    });
                }.frame(128,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection)
            }.mode(.horizontal).spacing(8)
            Button("Remove frame") {
                app.update_canvas(&path, |canvas| canvas.frames.retain(|frame| frame.id != id));
            }.frame(.infinity,26).alignment(.leading).padding(6).cornerRadius(4).hoverBackground(colors.selection).color(.red)
        }
        .padding(10)
        .spacing(4)
        .fontSize(12)
        .cornerRadius(9)
        .frame(284, 410)
        .background(colors.surface_container_high)
        .color(colors.on_surface)
    }
}

fn update_frame(
    app: &StudioEnvironment,
    path: &std::path::Path,
    id: u64,
    update: impl FnOnce(&mut VisualFrame),
) {
    app.update_canvas(path, |canvas| {
        if let Some(frame) = canvas.frames.iter_mut().find(|frame| frame.id == id) {
            update(frame);
        }
    });
}

fn move_frame(app: &StudioEnvironment, path: &std::path::Path, id: u64, delta: [i32; 2]) {
    update_frame(app, path, id, |frame| {
        for (position, step) in frame.position.iter_mut().zip(delta) {
            *position += step;
        }
    });
}

fn placement_scope(path: &std::path::Path, identity: u64, tree: &VisualTree) -> String {
    let start = tree.nodes.first().map_or(0, |node| node.range.start);
    format!("{}:{identity}:{start}", path.display())
}

fn reorder_sink(
    app: &StudioEnvironment,
    path: &std::path::Path,
    identity: u64,
    tree: &Arc<VisualTree>,
) -> render::Reorder {
    let app = app.clone();
    let path = path.to_owned();
    let tree = tree.clone();
    let resize_app = app.clone();
    let resize_path = path.clone();
    let resize_tree = tree.clone();
    render::Reorder {
        resize: Rc::new(move |index, dimensions| {
            resize_app.edit_visual(
                &resize_path,
                identity,
                &resize_tree,
                index,
                ComponentEdit::ResizeFixed(dimensions),
            );
        }),
        scope: placement_scope(&path, identity, &tree),
        move_node: std::rc::Rc::new(move |source, target, placement| {
            let source = tree
                .nodes
                .iter()
                .position(|node| node.name_range.start == source);
            let target = tree
                .nodes
                .iter()
                .position(|node| node.name_range.start == target);
            if let (Some(source), Some(target)) = (source, target) {
                app.edit_visual(
                    &path,
                    identity,
                    &tree,
                    source,
                    ComponentEdit::Place { target, placement },
                );
            }
        }),
    }
}

#[derive(Clone)]
struct VisualEdit {
    app: StudioEnvironment,
    path: PathBuf,
    identity: u64,
    tree: Arc<VisualTree>,
    target: usize,
}

impl VisualEdit {
    fn new(
        app: &StudioEnvironment,
        path: &std::path::Path,
        identity: u64,
        tree: &Arc<VisualTree>,
        target: usize,
    ) -> Self {
        Self {
            app: app.clone(),
            path: path.to_owned(),
            identity,
            tree: tree.clone(),
            target,
        }
    }

    fn apply(&self, edit: ComponentEdit) {
        self.app
            .edit_visual(&self.path, self.identity, &self.tree, self.target, edit);
    }
}
