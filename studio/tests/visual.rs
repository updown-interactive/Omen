use futures::{
    executor::{LocalPool, block_on},
    task::LocalSpawnExt,
};
use omen::{EditorEvent, Runtime, View, ViewKind, ViewNode};
use omen_studio::{
    app::StudioEnvironment,
    visual::{CanvasSettings, Component, ComponentEdit, EditorMode, Profile, VisualTree},
    workspace::Command,
};
use std::{fs, path::PathBuf};

const SOURCE: &str = r#"use omen::prelude::*;
view! {
    @Design
    pub Home {
        Layout {
            Text("Héllo 🌍")
            Button("Continue") { /* original action */ submit(); }
        }.padding(/* keep */24).horizontal(.medium)
    }
}
"#;

#[test]
fn composition_preserves_unrelated_source_modifiers_and_action_bodies() {
    let source = SOURCE.replace('\n', "\r\n");
    let tree = VisualTree::parse(&source).unwrap();
    assert_eq!(tree.parent(1), Some(0));
    assert_eq!(tree.selected(source.find("🌍").unwrap()), Some(1));

    let (patch, selection) = tree.edit(&source, 2, ComponentEdit::Move(true)).unwrap();
    let mut updated = source.clone();
    updated.replace_range(patch.range, &patch.replacement);
    assert!(updated[selection..].starts_with("Button"));
    assert!(updated.contains("/* original action */ submit();"));
    assert!(updated.contains(".padding(/* keep */24).horizontal(.medium)"));
    assert!(tree.edit(&updated, 1, ComponentEdit::Delete).is_err());

    let tree = VisualTree::parse(&updated).unwrap();
    let (patch, selection) = tree
        .edit(&updated, 0, ComponentEdit::Insert(Component::Scroll))
        .unwrap();
    updated.replace_range(patch.range, &patch.replacement);
    assert!(updated[selection..].starts_with("Layout {}.scrollable()\r\n"));
    assert_eq!(VisualTree::parse(&updated).unwrap().nodes.len(), 4);
}

#[test]
fn structural_edits_decline_ambiguous_or_unfinished_source() {
    let source = SOURCE.replace(
        "            Button",
        "            // associated with the button\n            Button",
    );
    let tree = VisualTree::parse(&source).unwrap();
    assert!(tree.edit(&source, 2, ComponentEdit::Move(true)).is_err());
    assert!(VisualTree::parse("view! { Home { Layout {").is_err());
    assert!(
        VisualTree::parse(&format!(
            "{SOURCE}\nview! {{ Other {{ Text(\"Other\") }} }}"
        ))
        .is_err()
    );
    assert!(VisualTree::parse("view! { Home { if show { Text(\"Dynamic\") } } }").is_err());
}

#[test]
fn canvas_geometry_preserves_logical_viewports_and_unique_frame_ids() {
    let mut canvas = CanvasSettings::default();
    canvas.add(Profile::WebDesktop, None);
    let second = canvas.frames[1].clone();
    canvas.add(second.profile, Some(second.clone()));
    assert_ne!(canvas.frames[2].id, second.id);
    assert_eq!(canvas.frames[2].width, second.width);
    assert!(canvas.frames[2].position[0] > second.position[0]);
    canvas.fit_to_view = false;
    canvas.center([900., 600.]);
    assert!(canvas.pan[0] > 0.);
    canvas.fit([900., 600.]);
    assert!(canvas.zoom < 100.);
    assert_eq!(canvas.pan, [0., 0.]);
    assert_eq!(canvas.frames[1].width, 1280);
    assert_eq!(canvas.frames[1].height, 800);
}

#[test]
fn fitting_a_tall_phone_tracks_the_canvas_without_altering_the_simulator_size() {
    let mut canvas = CanvasSettings::default();
    let viewport = [610., 440.];
    let fitted = canvas.visible_zoom(viewport);
    let [width, height] = canvas.extents(fitted / 100.);
    assert!(width <= viewport[0] && height <= viewport[1]);
    assert!(canvas.visible_zoom([610., 300.]) < fitted);
    canvas.zoom_by(viewport, 10);
    assert_eq!(canvas.visible_zoom([610., 300.]), fitted + 10.);
    assert_eq!(canvas.frames[0].height, 844);
}

#[test]
fn canvas_zoom_holds_the_anchor_and_fit_recovers_negative_frame_positions() {
    let mut canvas = CanvasSettings::default();
    let viewport = [640., 640.];
    let anchor = [160., 190.];
    let initial_scale = canvas.visible_zoom(viewport) / 100.;
    let before = canvas.origin(viewport, initial_scale);
    canvas.zoom_at(viewport, 110., anchor);
    let after = canvas.origin(viewport, 1.1);
    for axis in 0..2 {
        let world_before = (anchor[axis] - before[axis]) / initial_scale;
        let world_after = (anchor[axis] - after[axis]) / 1.1;
        assert!((world_before - world_after).abs() < 0.001);
    }
    canvas.pan_by(viewport, [500., 500.]);
    assert!(canvas.pan.iter().all(|pan| *pan < 0.));
    canvas.frames[0].position = [-500, -600];
    canvas.fit(viewport);
    let scale = canvas.visible_zoom(viewport) / 100.;
    let [x, y] = canvas.origin(viewport, scale);
    assert!(x - 500. * scale >= 18. && y - 600. * scale >= 18.);
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn find_nodes<'a>(
    node: &'a ViewNode,
    predicate: &impl Fn(&ViewNode) -> bool,
    found: &mut Vec<&'a ViewNode>,
) {
    if predicate(node) {
        found.push(node);
    }
    if let ViewKind::Layout(children)
    | ViewKind::Group(children)
    | ViewKind::Card(children)
    | ViewKind::List(children)
    | ViewKind::Presentation { children, .. } = &node.kind
    {
        for child in children {
            find_nodes(child, predicate, found);
        }
    }
}

#[test]
fn view_tabs_preview_one_buffer_and_restore_independent_frame_metadata() {
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-visual-test-{}", std::process::id())));
    let root = fixture.0.join("project");
    let profile = fixture.0.join("profile");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    let login = root.join("login_view.rs");
    let state_file = root.join("home_state.rs");
    fs::write(&home, SOURCE).unwrap();
    fs::write(&login, "view! { @Design Login { Text(\"Login\") } }").unwrap();
    fs::write(&state_file, "struct HomeState;").unwrap();

    let app = StudioEnvironment::at(profile.clone());
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root.clone()));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();

    let mut runtime = Runtime::new(app.editor_view().into_node());
    let has_editor = |node: &ViewNode| node.style.code_document.is_some();
    let first_render = runtime.render().unwrap();
    let mut editors = Vec::new();
    find_nodes(&first_render, &has_editor, &mut editors);
    assert!(editors.is_empty(), "view tabs initially open in Design");

    app.update_canvas(&home, |canvas| {
        canvas.mode = EditorMode::Split;
        canvas.add(Profile::WebDesktop, None);
    });
    let document = app.editor_document(home.clone());
    let text = document.state.get().text;
    block_on(app.analyze_visual(home.clone(), text.clone()));
    let rendered = runtime.render().unwrap();
    find_nodes(&rendered, &has_editor, &mut editors);
    assert_eq!(editors.len(), 1, "Split embeds the original code editor");

    let frame_scales = |root: &ViewNode| {
        let mut frames = Vec::new();
        find_nodes(
            root,
            &|node| {
                node.style
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.scale.is_some())
            },
            &mut frames,
        );
        frames
            .into_iter()
            .map(|node| node.style.preview.as_ref().unwrap().scale.unwrap())
            .collect::<Vec<_>>()
    };
    let initial_scales = frame_scales(&rendered);
    assert_eq!(initial_scales.len(), 2);
    assert!(initial_scales.iter().all(|&scale| scale < 1.5));
    app.update_canvas(&home, |canvas| {
        canvas.zoom_at([320., 500.], 150., [10., 10.]);
    });
    assert_eq!(frame_scales(&runtime.render().unwrap()), [1.5, 1.5]);
    assert!(
        runtime
            .last_rebuilt()
            .iter()
            .all(|view| !view.ends_with("ViewTree") && !view.ends_with("ViewTreeRow")),
        "camera changes should reuse the floating tree’s view recipe"
    );
    let mut labels = Vec::new();
    find_nodes(
        &rendered,
        &|node| matches!(&node.kind, ViewKind::Text(text) if text == "Héllo 🌍"),
        &mut labels,
    );
    assert_eq!(
        labels.len(),
        2,
        "both frames contain actual Omen Text views"
    );
    labels[1].style.on_tap.as_ref().unwrap().invoke();
    let cursor = document.state.get().selection.end;
    assert_eq!(cursor, SOURCE.find("Text").unwrap());
    block_on(app.inspector_task(home.clone(), text.clone(), cursor)());
    assert_eq!(app.inspector.get().node.unwrap().component, "Text");

    let cursor = SOURCE.find("Layout").unwrap();
    document
        .state
        .update(|state| state.selection = cursor..cursor);
    block_on(app.inspector_task(home.clone(), text, cursor)());
    let inspected = app.inspector.get();
    let padding = inspected
        .node
        .unwrap()
        .properties
        .into_iter()
        .find(|property| property.name == "padding")
        .unwrap();
    let viewport_keys = |root: &ViewNode| {
        let mut nodes = Vec::new();
        find_nodes(
            root,
            &|node| {
                node.style
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.width.is_some())
            },
            &mut nodes,
        );
        nodes
            .into_iter()
            .map(|node| node.style.runtime_id.clone())
            .collect::<Vec<_>>()
    };
    let before_keys = viewport_keys(&runtime.render().unwrap());
    assert_eq!(before_keys.len(), 2);
    app.inspector_set(&padding.key, "32", false, inspected.revision);
    let updated = document.state.get().text;
    let pending = runtime.render().unwrap();
    assert_eq!(
        viewport_keys(&pending),
        before_keys,
        "pending analysis keeps both simulators mounted"
    );
    block_on(app.analyze_visual(home.clone(), updated.clone()));
    let rendered = runtime.render().unwrap();
    assert_eq!(
        viewport_keys(&rendered),
        before_keys,
        "new render updates the mounted frames"
    );
    let mut layouts = Vec::new();
    find_nodes(
        &rendered,
        &|node| {
            node.style
                .preview
                .as_ref()
                .is_some_and(|preview| preview.selected)
                && node.style.padding == Some(32.)
        },
        &mut layouts,
    );
    assert_eq!(layouts.len(), 2);
    assert_eq!(layouts[0].style.for_width(390.).mode, None);
    assert_eq!(
        layouts[1].style.for_width(1280.).mode,
        Some(omen::LayoutMode::Horizontal)
    );
    assert_eq!(fs::read_to_string(&home).unwrap(), SOURCE);
    app.visual_undo(&home, false);
    assert_eq!(document.state.get().text.to_string(), SOURCE);
    app.visual_undo(&home, true);
    assert!(document.state.get().text.same_snapshot(&updated));
    let mut typed = updated.clone();
    let start = typed.to_string().find("Héllo 🌍").unwrap();
    typed.replace(start..start + "Héllo 🌍".len(), "Updated from Code");
    document.state.update(|state| state.text = typed.clone());
    document.emit(EditorEvent::Edited);
    block_on(app.analyze_visual(home.clone(), typed));
    let rendered = runtime.render().unwrap();
    let mut updated_labels = Vec::new();
    find_nodes(
        &rendered,
        &|node| matches!(&node.kind, ViewKind::Text(text) if text == "Updated from Code"),
        &mut updated_labels,
    );
    assert_eq!(updated_labels.len(), 2);
    let text = document.state.get().text;
    let cursor = document.state.get().selection.end;
    block_on(app.inspector_task(home.clone(), text, cursor)());
    app.inspector_add("background", app.inspector.get().revision);
    let modifier_edit = document.state.get().text;
    assert!(modifier_edit.to_string().contains(".background("));
    assert_eq!(viewport_keys(&runtime.render().unwrap()), before_keys);
    block_on(app.analyze_visual(home.clone(), modifier_edit.clone()));
    assert_eq!(viewport_keys(&runtime.render().unwrap()), before_keys);
    let mut invalid = modifier_edit.clone();
    invalid.replace(invalid.len()..invalid.len(), "{");
    document.state.update(|state| state.text = invalid.clone());
    block_on(app.analyze_visual(home.clone(), invalid));
    assert_eq!(viewport_keys(&runtime.render().unwrap()), before_keys);
    document.state.update(|state| state.text = modifier_edit);
    app.send(Command::Save(home.clone()));
    app.send(Command::OpenFile(login.clone()));
    app.flush();
    pool.run_until_stalled();
    assert!(app.workspace.get().canvas(&login).is_none());
    app.send(Command::OpenFile(state_file));
    app.flush();
    pool.run_until_stalled();
    let rendered = runtime.render().unwrap();
    let mut editors = Vec::new();
    find_nodes(&rendered, &has_editor, &mut editors);
    assert_eq!(editors.len(), 1, "state files still open directly in Code");

    app.send(Command::RenameEntry {
        path: home,
        name: "dashboard_view.rs".into(),
    });
    app.flush();
    pool.run_until_stalled();
    let restored = omen_studio::services::read_json::<omen_studio::state::WorkspaceState>(
        &profile.join("workspace.json"),
    )
    .unwrap();
    let dashboard = root.join("dashboard_view.rs");
    assert_eq!(restored.canvas(&dashboard).unwrap().mode, EditorMode::Split);
    assert_eq!(restored.canvas(&dashboard).unwrap().frames.len(), 2);
    assert!(
        fs::read_to_string(&dashboard)
            .unwrap()
            .contains("/* keep */32")
    );
}

#[gpui::test]
fn dragging_and_zooming_the_mounted_studio_workspace(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseButton, ScrollDelta, ScrollEvent, point, px};
    use omen_gpui::OmenWindow;

    // The mounted workspace wakes GPUI from its filesystem worker.
    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-native-workbench-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    fs::write(&home, SOURCE).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();
    app.update_canvas(&home, |canvas| {
        canvas.fit_to_view = false;
        canvas.pan = [-6000., -400.];
        canvas.frames[0].position = [4300, 2100];
    });
    let text = app.editor_document(home.clone()).state.get().text;
    block_on(app.analyze_visual(home.clone(), text));

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    assert!(app.canvas_settings(&home).fit_to_view);
    let workspace_sequence = app.workspace.get().sequence;
    let surface = vcx.debug_bounds("studio-canvas").expect("canvas viewport");
    let before = vcx.debug_bounds("studio-frame-grip").expect("frame header");
    let initial = app.canvas_settings(&home);
    let start = before.center();
    assert!(
        surface.contains(&start),
        "header must begin on the visible canvas"
    );
    let end = start + point(px(28.), px(42.));
    vcx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    vcx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    vcx.run_until_parked();
    let moved = app.canvas_settings(&home);
    assert_ne!(
        initial.frames[0].position, moved.frames[0].position,
        "drag must move the artboard"
    );
    let after = vcx.debug_bounds("studio-frame-grip").unwrap();
    assert!(
        (f32::from(after.origin.x - before.origin.x) - 28.).abs() <= 2.,
        "{before:?} -> {after:?}; surface {surface:?}; settings {:?}",
        app.canvas_settings(&home)
    );
    assert!((f32::from(after.origin.y - before.origin.y) - 42.).abs() <= 2.);

    let anchor = after.center();
    vcx.simulate_event(ScrollEvent {
        position: anchor,
        delta: ScrollDelta::Pixels(point(px(0.), px(36.))),
        modifiers: gpui::Modifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    });
    vcx.run_until_parked();
    let zoomed = vcx.debug_bounds("studio-frame-grip").unwrap();
    assert!(
        zoomed.size.height > after.size.height,
        "{after:?} -> {zoomed:?}"
    );
    assert!((f32::from(zoomed.center().x - anchor.x)).abs() <= 2.);
    assert!((f32::from(zoomed.center().y - anchor.y)).abs() <= 2.);

    let background = surface.origin + point(px(24.), px(32.));
    let destination = background + point(px(36.), px(24.));
    vcx.simulate_mouse_down(background, MouseButton::Left, Default::default());
    for delta in [
        point(px(12.), px(8.)),
        point(px(8.), px(6.)),
        point(px(31.), px(19.)),
    ] {
        vcx.simulate_mouse_move(background + delta, MouseButton::Left, Default::default());
    }
    vcx.simulate_mouse_up(destination, MouseButton::Left, Default::default());
    vcx.run_until_parked();
    let panned = vcx.debug_bounds("studio-frame-grip").unwrap();
    assert!((f32::from(panned.origin.x - zoomed.origin.x) - 36.).abs() <= 2.);
    assert!((f32::from(panned.origin.y - zoomed.origin.y) - 24.).abs() <= 2.);
    assert_eq!(app.workspace.get().sequence, workspace_sequence);
    let before_pinch = app.canvas_settings(&home).zoom;
    app.canvas_navigation(home.clone(), [640., 640.])
        .set(omen::PreviewGesture::Zoom {
            factor: 1.002,
            anchor: [320., 320.],
        });
    let after_pinch = app.canvas_settings(&home).zoom;
    assert!(after_pinch > before_pinch && after_pinch < before_pinch + 1.);
    app.flush();
    let saved = omen_studio::services::read_json::<omen_studio::state::WorkspaceState>(
        &fixture.0.join("profile/workspace.json"),
    )
    .unwrap();
    assert_eq!(saved.canvas(&home).unwrap().zoom, after_pinch);
    app.update_canvas(&home, |canvas| canvas.pan_by([640., 640.], [6000., 0.]));
    vcx.run_until_parked();
    let canvas = app.canvas_settings(&home);
    assert!(!canvas.fit_to_view);
    assert!(!canvas.has_visible_frame([640., 640.]));
    assert!(vcx.debug_bounds("studio-frame-card").is_none());

    for profile in Profile::ALL {
        let (width, height) = profile.size();
        app.update_canvas(&home, |canvas| {
            canvas.zoom = 50.;
            canvas.pan = [0., 0.];
            let frame = &mut canvas.frames[0];
            frame.position = [10, 10];
            frame.profile = profile;
            (frame.width, frame.height) = (width, height);
        });
        vcx.run_until_parked();
        let shell = vcx.debug_bounds("studio-device-shell").unwrap();
        let screen = vcx.debug_bounds("studio-device-screen").unwrap();
        let [side, top, bottom] = profile.shell().insets();
        assert_eq!(screen.size.width, px(f32::from(width) * 0.5), "{profile:?}");
        assert_eq!(
            screen.size.height,
            px(f32::from(height) * 0.5),
            "{profile:?}"
        );
        // GPUI rounds the two bounds independently to the pixel grid.
        for (actual, expected) in [
            (screen.left() - shell.left(), side),
            (screen.top() - shell.top(), top),
            (shell.bottom() - screen.bottom(), bottom),
        ] {
            assert!(
                (f32::from(actual) - expected * 0.5).abs() <= 1.,
                "{profile:?} {screen:?} {shell:?}"
            );
        }
    }

    app.update_canvas(&home, |canvas| {
        canvas.zoom = 50.;
        canvas.pan = [0., 0.];
        canvas.add(Profile::IPhone, None);
        for (i, frame) in canvas.frames.iter_mut().enumerate() {
            frame.width = 160;
            frame.height = 120;
            frame.position = [20 + i as i32 * 220, 140];
        }
    });
    vcx.run_until_parked();
    for (index, x) in [(0, 34.), (1, 144.)] {
        let before = app.canvas_settings(&home);
        // Grab the caption below the title grip, outside the component preview.
        let press = surface.origin + point(px(x), px(88.));
        let release = press + point(px(12.), px(9.));
        vcx.simulate_mouse_down(press, MouseButton::Left, Default::default());
        vcx.simulate_mouse_move(release, MouseButton::Left, Default::default());
        vcx.simulate_mouse_up(release, MouseButton::Left, Default::default());
        vcx.run_until_parked();
        let after = app.canvas_settings(&home);
        let [x, y] = before.frames[index].position;
        assert_eq!(after.frames[index].position, [x + 24, y + 18]);
        assert_eq!(after.active_frame, after.frames[index].id);
        assert_eq!(after.frames[1 - index], before.frames[1 - index]);
        assert_eq!(after.pan, before.pan);
    }
}

#[test]
fn flow_placement_reparents_inserts_wraps_and_resizes_real_source() {
    use omen::FlowPlacement;
    let source = r#"view! {
        Home {
            Layout {
                Layout {
                    Button("Continue") { /* action */ submit(); }.frame(140, 40)
                    Text("Title")
                }
                Layout {}
            }
        }
    }"#;
    let apply = |source: &str, node: usize, edit| {
        let tree = VisualTree::parse(source).unwrap();
        let (patch, cursor) = tree.edit(source, node, edit).unwrap();
        let mut edited = source.to_owned();
        edited.replace_range(patch.range, &patch.replacement);
        (edited, cursor)
    };
    let tree = VisualTree::parse(source).unwrap();
    assert!(
        tree.edit(
            source,
            1,
            ComponentEdit::Place {
                target: 2,
                placement: FlowPlacement::Into
            }
        )
        .is_err()
    );

    let (moved, selected) = apply(
        source,
        2,
        ComponentEdit::Place {
            target: 4,
            placement: FlowPlacement::Into,
        },
    );
    assert!(moved[selected..].starts_with("Button"));
    assert!(moved.contains("/* action */ submit();"));
    let tree = VisualTree::parse(&moved).unwrap();
    let button = tree.selected(selected).unwrap();
    assert_eq!(tree.parent(button), Some(3));

    let (resized, _) = apply(&moved, button, ComponentEdit::ResizeFixed([176., 54.]));
    assert!(resized.contains(".frame(176, 54)"));
    let (inserted, selected) = apply(
        &resized,
        button,
        ComponentEdit::InsertAt {
            target: button,
            placement: FlowPlacement::Before,
            component: Component::Text,
        },
    );
    assert!(inserted[selected..].starts_with(r#"Text("Text")"#));
    let tree = VisualTree::parse(&inserted).unwrap();
    let child = tree.selected(selected).unwrap();
    let (wrapped, selected) = apply(&inserted, child, ComponentEdit::Wrap);
    assert!(wrapped[selected..].starts_with("Layout {"));
    assert!(wrapped.contains("/* action */ submit();"));
}

#[gpui::test]
fn mounted_phone_drops_button_before_description_and_undoes_the_source_edit(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::{MouseButton, point, px};
    use omen_gpui::OmenWindow;
    cx.executor().allow_parking();
    let fixture = Fixture(
        std::env::temp_dir().join(format!("omen-placement-workbench-{}", std::process::id())),
    );
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let source = r#"view! {
    @Design
    Home {
        Layout {
            Text("Title")
            Text("Description")
            Button("Continue") { /* keep action */ submit(); }.frame(140, 40)
        }.padding(24).spacing(16)
    }
}"#;
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    block_on(app.analyze_visual(path.clone(), document.state.get().text));

    let selector = |text| -> &'static str {
        let offset = source.find(text).unwrap();
        Box::leak(format!("studio-placement-{offset}").into_boxed_str())
    };
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let source_bounds = vcx.debug_bounds(selector("Button")).unwrap();
    let target_bounds = vcx.debug_bounds(selector("Text(\"Description\")")).unwrap();
    let from = source_bounds.center();
    let to = point(target_bounds.center().x, target_bounds.origin.y + px(2.));
    vcx.simulate_mouse_down(from, MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(
        from - point(px(0.), px(10.)),
        MouseButton::Left,
        Default::default(),
    );
    vcx.simulate_mouse_move(to, MouseButton::Left, Default::default());
    assert_eq!(document.state.get().text.to_string(), source);
    vcx.simulate_mouse_up(to, MouseButton::Left, Default::default());

    let edited = document.state.get().text.to_string();
    assert!(
        edited.find("Button").unwrap() < edited.find("Description").unwrap(),
        "{edited}"
    );
    assert!(edited.contains("/* keep action */ submit();"));
    assert!(edited[document.state.get().selection.end..].starts_with("Button"));
    app.visual_undo(&path, false);
    assert_eq!(document.state.get().text.to_string(), source);
    app.visual_undo(&path, true);
    assert_eq!(document.state.get().text.to_string(), edited);
    block_on(app.analyze_visual(path.clone(), document.state.get().text));
    vcx.run_until_parked();
    let handle = vcx.debug_bounds("studio-resize-handle").unwrap().center();
    let end = handle + point(px(22.), px(18.));
    vcx.simulate_mouse_down(handle, MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(
        handle + point(px(8.), px(6.)),
        MouseButton::Left,
        Default::default(),
    );
    vcx.simulate_mouse_move(end, MouseButton::Left, Default::default());
    assert_eq!(document.state.get().text.to_string(), edited);
    vcx.simulate_mouse_up(end, MouseButton::Left, Default::default());
    let resized = document.state.get().text.to_string();
    assert_ne!(resized, edited, "a resize commits on release");
    let tree = VisualTree::parse(&resized).unwrap();
    let selected = tree.selected(document.state.get().selection.end).unwrap();
    assert_eq!(tree.nodes[selected].component, "Button");
    assert!(
        tree.nodes[selected]
            .properties
            .iter()
            .any(|property| property.name == "frame" && property.value != "140, 40")
    );
    app.visual_undo(&path, false);
    assert_eq!(document.state.get().text.to_string(), edited);
}

#[gpui::test]
fn mounted_phone_moves_a_pinned_button_inside_its_layout(cx: &mut gpui::TestAppContext) {
    use gpui::{MouseButton, point, px};
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-pin-workbench-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let source = r#"view! {
    @Design
    Home {
        Layout {
            Button("Continue") { /* keep action */ submit(); }.frame(100, 40)
            Text("Description").frame(.infinity, .infinity)
        }
    }
}"#;
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();

    let document = app.editor_document(path.clone());
    let tree = VisualTree::parse(source).unwrap();
    app.edit_visual(
        &path,
        document.state.get().text.identity(),
        &tree,
        1,
        ComponentEdit::TogglePin,
    );
    let pinned = document.state.get().text.to_string();
    assert!(pinned.contains(".pin(.topLeading, 0, 0)"), "{pinned}");
    assert!(pinned.contains("}.frame(.infinity, .infinity)"));
    block_on(app.analyze_visual(path.clone(), document.state.get().text));

    let selector = |text| -> &'static str {
        let offset = pinned.find(text).unwrap();
        Box::leak(format!("studio-placement-{offset}").into_boxed_str())
    };
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let from = vcx.debug_bounds(selector("Button")).unwrap().center();
    let layout = vcx.debug_bounds(selector("Layout")).unwrap();
    let to = layout.center() + point(layout.size.width / 4., layout.size.height / 4.);
    assert!(vcx.debug_bounds("studio-canvas").unwrap().contains(&to));
    let sibling = vcx.debug_bounds(selector("Text")).unwrap();
    assert!(sibling.contains(&to), "{sibling:?} does not cover {to:?}");
    vcx.simulate_mouse_down(from, MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(
        from + point(px(8.), px(8.)),
        MouseButton::Left,
        Default::default(),
    );
    vcx.simulate_mouse_move(to, MouseButton::Left, Default::default());
    assert_eq!(document.state.get().text.to_string(), pinned);
    vcx.simulate_mouse_up(to, MouseButton::Left, Default::default());

    let moved = document.state.get().text.to_string();
    assert!(moved.contains(".pin(.bottomTrailing,"), "{moved}");
    assert!(moved.contains("/* keep action */ submit();"));
    block_on(app.analyze_visual(path.clone(), document.state.get().text));
    vcx.run_until_parked();
    let position = vcx.debug_bounds(selector("Button")).unwrap().center();
    assert!((position - to).magnitude() <= 2., "{position:?} != {to:?}");
    app.visual_undo(&path, false);
    assert_eq!(document.state.get().text.to_string(), pinned);
    app.visual_undo(&path, false);
    assert_eq!(document.state.get().text.to_string(), source);
}

#[test]
fn declarative_pin_compiles_and_can_return_to_flow() {
    use omen::prelude::*;

    let button = view! { Button("Continue") {}.pin(.bottomTrailing, 24, 32) }.into_node();
    assert_eq!(
        button.style.pin,
        Some(Pin {
            corner: PinCorner::BottomTrailing,
            x: 24,
            y: 32
        })
    );

    let source = r#"view! { Home { Layout { Button("Continue") {}.pin(.bottomTrailing, 24, 32) }.fill() } }"#;
    let tree = VisualTree::parse(source).unwrap();
    let (patch, _) = tree.edit(source, 1, ComponentEdit::TogglePin).unwrap();
    let mut text = source.to_owned();
    text.replace_range(patch.range, &patch.replacement);
    assert_eq!(
        text,
        r#"view! { Home { Layout { Button("Continue") {} }.fill() } }"#
    );
}

#[test]
fn component_chooser_guides_follow_the_active_frame_and_insertion_slot() {
    use omen::FlowPlacement;

    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-layout-guides-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    let source = r#"view! {
    @Design
    Home {
        Layout {
            Text("First")
            Text("Last")
        }.mode(.horizontal).alignment(.bottomTrailing).padding(16)
    }
}"#;
    fs::write(&home, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();

    let document = app.editor_document(home.clone());
    let first = source.find("Text(\"First\")").unwrap();
    let last = source.find("Text(\"Last\")").unwrap();
    document
        .state
        .update(|state| state.selection = first..first);
    block_on(app.analyze_visual(home.clone(), document.state.get().text));
    app.update_canvas(&home, |canvas| canvas.add(Profile::WebDesktop, None));
    let mut runtime = Runtime::new(app.editor_view().into_node());

    let click = |root: &ViewNode, label: &str| {
        let mut buttons = Vec::new();
        find_nodes(
            root,
            &|node| matches!(&node.kind, ViewKind::Button { label: title, .. } if title == label),
            &mut buttons,
        );
        let [button] = buttons.as_slice() else {
            panic!("expected exactly one {label} button");
        };
        let ViewKind::Button { action, .. } = &button.kind else {
            unreachable!()
        };
        action.invoke();
    };
    let markers = |root: &ViewNode| {
        let mut layouts = Vec::new();
        find_nodes(
            root,
            &|node| {
                node.style
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.layout_guide)
            },
            &mut layouts,
        );
        let mut nodes = Vec::new();
        find_nodes(
            root,
            &|node| {
                node.style
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.insertion_edge.is_some())
            },
            &mut nodes,
        );
        let markers = nodes
            .into_iter()
            .map(|node| {
                let preview = node.style.preview.as_ref().unwrap();
                (
                    preview.placement.as_ref().unwrap().source.start,
                    preview.insertion_edge.unwrap(),
                )
            })
            .collect::<Vec<_>>();
        (layouts.len(), markers)
    };

    let closed = runtime.render().unwrap();
    assert_eq!(markers(&closed), (0, vec![]));
    click(&closed, "+ Add component");
    let opened = runtime.render().unwrap();
    let mut labels = Vec::new();
    find_nodes(
        &opened,
        &|node| matches!(&node.kind, ViewKind::Text(label) if label == "Row · Bottom trailing"),
        &mut labels,
    );
    assert_eq!(labels.len(), 1);
    assert_eq!(markers(&opened), (1, vec![(last, FlowPlacement::After)]));

    click(&opened, "Before");
    let before = runtime.render().unwrap();
    assert_eq!(markers(&before), (1, vec![(first, FlowPlacement::Before)]));
    click(&before, "After");
    let after = runtime.render().unwrap();
    assert_eq!(markers(&after), (1, vec![(first, FlowPlacement::After)]));

    click(&after, "+ Add component");
    assert_eq!(markers(&runtime.render().unwrap()), (0, vec![]));
    assert_eq!(document.state.get().text.to_string(), source);

    click(&runtime.render().unwrap(), "Text  ▾");
    click(&runtime.render().unwrap(), "Layout · Row");
    assert_eq!(
        document.state.get().selection.start,
        source.find("Layout").unwrap()
    );
    click(&runtime.render().unwrap(), "Layout · Row  ▾");
    click(&runtime.render().unwrap(), "Text");
    assert_eq!(document.state.get().selection.start, first);

    click(&runtime.render().unwrap(), "Edit selection  ▾");
    let commands = runtime.render().unwrap();
    let mut unavailable = Vec::new();
    find_nodes(
        &commands,
        &|node| matches!(&node.kind, ViewKind::Button { label, .. } if label.starts_with("Move before")),
        &mut unavailable,
    );
    assert!(unavailable.is_empty());
    click(&commands, "Move after Text");
    let reordered = document.state.get().text.to_string();
    assert!(reordered.find("Last").unwrap() < reordered.find("First").unwrap());
}

#[gpui::test]
fn frame_settings_and_profile_menus_fit_at_window_scale(cx: &mut gpui::TestAppContext) {
    use gpui::{ScrollDelta, ScrollEvent, point, px, size};
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-frame-menu-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    fs::write(&home, SOURCE).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();

    let document = app.editor_document(home.clone());
    block_on(app.analyze_visual(home.clone(), document.state.get().text));
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let surface = vcx.debug_bounds("studio-canvas").unwrap();
    let viewport = [surface.size.width.into(), surface.size.height.into()];

    for (profile, zoom) in [(Profile::IPhone, 70.), (Profile::IPad, 45.)] {
        app.update_canvas(&home, |canvas| {
            let frame = &mut canvas.frames[0];
            frame.profile = profile;
            (frame.width, frame.height) = profile.size();
            canvas.zoom = zoom;
            canvas.center(viewport);
        });
        vcx.run_until_parked();
        let settings = app.canvas_settings(&home);
        let scale = settings.visible_zoom(viewport) / 100.;
        let grip = vcx.debug_bounds("studio-frame-grip").unwrap();
        let trigger = point(grip.right() + px(16.5 * scale), grip.center().y);
        assert!(surface.contains(&trigger));
        vcx.simulate_click(trigger, Default::default());
        vcx.run_until_parked();
        let panel = vcx
            .debug_bounds("studio-menu-panel")
            .expect("Frame settings");
        assert_eq!(panel.size, size(px(286.), px(412.)));

        vcx.simulate_event(ScrollEvent {
            position: panel.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-80.))),
            ..Default::default()
        });
        vcx.run_until_parked();
        assert_eq!(vcx.debug_bounds("studio-menu-panel"), Some(panel));
        assert_eq!(app.canvas_settings(&home).pan, settings.pan);

        let profile_trigger = panel.origin + point(px(24.), px(48.));
        vcx.simulate_click(profile_trigger, Default::default());
        vcx.run_until_parked();
        let profiles = vcx
            .debug_bounds("studio-menu-panel")
            .expect("frame profile menu");
        assert_eq!(profiles.size, size(px(202.), px(337.)));
        let last_profile = profiles.bottom_left() + point(px(32.), px(-24.));
        vcx.simulate_click(last_profile, Default::default());
        vcx.run_until_parked();
        assert_eq!(
            app.canvas_settings(&home).frames[0].profile,
            Profile::Custom
        );
        vcx.simulate_keystrokes("escape");
        vcx.run_until_parked();
        assert!(vcx.debug_bounds("studio-menu-panel").is_none());
    }
}

#[gpui::test]
fn backspace_deletes_the_focused_design_component_and_can_be_undone(cx: &mut gpui::TestAppContext) {
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-backspace-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    fs::write(&path, SOURCE).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    block_on(app.analyze_visual(path.clone(), document.state.get().text));

    let offset = SOURCE.find("Button").unwrap();
    let selector = Box::leak(format!("studio-placement-{offset}").into_boxed_str());
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let button = vcx.debug_bounds(selector).unwrap().center();
    vcx.simulate_click(button, Default::default());
    vcx.run_until_parked();
    assert_eq!(document.state.get().selection, offset..offset);

    vcx.simulate_keystrokes("backspace");
    vcx.run_until_parked();
    let text = document.state.get().text.to_string();
    let tree = VisualTree::parse(&text).unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .map(|node| node.component.as_str())
            .collect::<Vec<_>>(),
        ["Layout", "Text"],
    );
    let parent = SOURCE.find("Layout").unwrap();
    assert_eq!(document.state.get().selection, parent..parent);

    app.visual_undo(&path, false);
    assert_eq!(document.state.get().text.to_string(), SOURCE);

    app.set_visual_mode(EditorMode::Split);
    let label_end = SOURCE.find("Continue").unwrap() + "Continue".len();
    document.state.update(|state| {
        state.selection = label_end..label_end;
        state.selection_request = Some(label_end..label_end);
    });
    app.focus_editor();
    vcx.run_until_parked();
    vcx.simulate_keystrokes("backspace");
    vcx.run_until_parked();
    assert_eq!(
        document.state.get().text.to_string(),
        SOURCE.replace("Continue", "Continu")
    );
}

#[gpui::test]
fn floating_view_tree_selects_components_and_reflects_inspector_names(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::{ScrollDelta, ScrollEvent, point, px};
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-view-tree-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let more_rows = (1..=24)
        .map(|index| format!("            Text(\"Layer {index}\")\n"))
        .collect::<String>();
    let source = SOURCE.replace(
        "        }.padding",
        &format!("{more_rows}        }}.padding"),
    );
    fs::write(&path, &source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    block_on(app.analyze_visual(path.clone(), document.state.get().text));

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let panel = vcx
        .debug_bounds("studio-floating-panel")
        .expect("floating view tree");
    let surface = vcx.debug_bounds("studio-canvas").unwrap();
    assert_eq!(surface.right() - panel.right(), px(10.));

    let button_row = panel.origin + point(px(80.), px(102.));
    vcx.simulate_click(button_row, Default::default());
    vcx.run_until_parked();
    let offset = source.find("Button").unwrap();
    assert_eq!(document.state.get().selection, offset..offset);

    block_on(app.inspector_task(
        path.clone(),
        document.state.get().text,
        offset,
    )());
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let inspector = runtime.render().unwrap();
    let mut fields = Vec::new();
    find_nodes(
        &inspector,
        &|node| matches!(&node.kind, ViewKind::TextField { label, .. } if label == "Name"),
        &mut fields,
    );
    let [field] = fields.as_slice() else {
        panic!("expected the Inspector Name field");
    };
    let renamed = "Main \"Continue\" 🚀";
    field
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set(renamed.to_owned());
    let mut buttons = Vec::new();
    find_nodes(
        &inspector,
        &|node| matches!(&node.kind, ViewKind::Button { label, .. } if label == "Set name"),
        &mut buttons,
    );
    let [apply] = buttons.as_slice() else {
        panic!("expected the Set name button");
    };
    let ViewKind::Button { action, .. } = &apply.kind else {
        unreachable!()
    };
    action.invoke();

    let state = document.state.get();
    assert!(
        state
            .text
            .to_string()
            .contains(&format!(".name({renamed:?})"))
    );
    assert_eq!(state.selection, offset..offset);
    assert_eq!(
        app.inspector.get().node.unwrap().design_name().as_deref(),
        Some(renamed)
    );
    block_on(app.analyze_visual(path.clone(), state.text));
    let updated = runtime.render().unwrap();
    let mut rows = Vec::new();
    find_nodes(
        &updated,
        &|node| node.style.accessibility.label.as_deref() == Some(renamed),
        &mut rows,
    );
    assert_eq!(
        rows.len(),
        1,
        "the renamed view appears in the floating tree"
    );

    let mut buttons = Vec::new();
    find_nodes(
        &updated,
        &|node| {
            node.style
                .preview
                .as_ref()
                .and_then(|preview| preview.placement.as_ref())
                .is_some_and(|placement| placement.source.start == offset)
        },
        &mut buttons,
    );
    let [button] = buttons.as_slice() else {
        panic!("expected the simulator button");
    };
    assert!(button.style.preview.as_ref().unwrap().selected);

    vcx.run_until_parked();
    let panel = vcx.debug_bounds("studio-floating-panel").unwrap();
    let pan = app.canvas_settings(&path).pan;
    vcx.simulate_event(ScrollEvent {
        position: panel.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
        ..Default::default()
    });
    vcx.run_until_parked();
    assert_eq!(app.canvas_settings(&path).pan, pan);
    vcx.simulate_click(
        panel.bottom_center() - point(px(0.), px(18.)),
        Default::default(),
    );
    vcx.run_until_parked();
    let state = document.state.get();
    assert!(state.text.to_string()[state.selection.end..].starts_with("Text(\"Layer 24\")"));
}

#[test]
fn view_names_compile_as_metadata_independent_of_identity() {
    use omen::prelude::*;

    let node = view! { Text("Visible content").name("Layer label").key("stable") }.into_node();
    assert_eq!(node.style.name.as_deref(), Some("Layer label"));
    assert_eq!(node.style.key.as_deref(), Some("stable"));
    assert!(matches!(node.kind, ViewKind::Text(text) if text == "Visible content"));
}

#[test]
fn appending_and_deleting_gap_clean_only_the_edited_spacing() {
    use omen::FlowPlacement;

    let source = concat!(
        "use omen::prelude::*;\r\n\r\n",
        "view! {\r\n",
        "    Home {\r\n",
        "        Layout {\r\n",
        "            Text(\"Keep\") // keep this comment\r\n",
        "          \r\n",
        "        }\r\n",
        "    }\r\n",
        "}\r\n",
    );
    let tree = VisualTree::parse(source).unwrap();
    let (patch, selected) = tree
        .edit(source, 0, ComponentEdit::Insert(Component::Gap))
        .unwrap();
    let mut updated = source.to_owned();
    updated.replace_range(patch.range, &patch.replacement);
    assert!(updated[selected..].starts_with("Gap(8)"));
    assert!(
        updated.contains("Text(\"Keep\") // keep this comment\r\n            Gap(8)\r\n        }")
    );

    let dirty = updated.replace(
        "            Gap(8)\r\n",
        "         \r\n            Gap(8)\r\n           \r\n",
    );
    let tree = VisualTree::parse(&dirty).unwrap();
    let (patch, _) = tree.edit(&dirty, 2, ComponentEdit::Delete).unwrap();
    let mut cleaned = dirty;
    cleaned.replace_range(patch.range, &patch.replacement);
    assert_eq!(cleaned, source.replace("          \r\n", ""));

    let tree = VisualTree::parse(&cleaned).unwrap();
    let (patch, selected) = tree
        .edit(
            &cleaned,
            1,
            ComponentEdit::InsertAt {
                target: 1,
                placement: FlowPlacement::Before,
                component: Component::Gap,
            },
        )
        .unwrap();
    cleaned.replace_range(patch.range, &patch.replacement);
    assert!(cleaned[selected..].starts_with("Gap(8)\r\n            Text"));
}

#[gpui::test]
fn preview_gap_takes_space_on_the_parent_axis(cx: &mut gpui::TestAppContext) {
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture = Fixture(std::env::temp_dir().join(format!("omen-gap-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let source = r#"view! {
        @Design
        Home {
            Layout {
                Layout { Text("Left") Gap(17) Text("Right") }.mode(.horizontal)
                Layout { Text("Top") Gap() Text("Bottom") }
            }
        }
    }"#;
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    block_on(app.analyze_visual(path, document.state.get().text));

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();
    let scale = f32::from(vcx.debug_bounds("studio-frame-grip").unwrap().size.height) / 20.;
    let selector = |text| -> &'static str {
        Box::leak(format!("studio-placement-{}", source.find(text).unwrap()).into_boxed_str())
    };
    let row_gap = vcx.debug_bounds(selector("Gap(17)")).unwrap();
    let column_gap = vcx.debug_bounds(selector("Gap()")).unwrap();
    assert!((f32::from(row_gap.size.width) - 17. * scale).abs() <= 1.);
    assert!((f32::from(column_gap.size.height) - 8. * scale).abs() <= 1.);
}

#[gpui::test]
fn split_preview_follows_code_edits_across_view_declarations(cx: &mut gpui::TestAppContext) {
    use omen_gpui::OmenWindow;
    use std::time::{Duration, Instant};

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-live-code-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let source = concat!(
        "view! { @Design House { Layout { Text(\"First\").frame(80, 40) } } }\n",
        "view! { Header { Layout { Text(\"Second\").frame(128, 40) } } }\n",
    );
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    app.set_visual_mode(EditorMode::Split);

    let document = app.editor_document(path.clone());
    let selector = |text| -> &'static str {
        let offset = source.find(text).unwrap();
        Box::leak(format!("studio-placement-{offset}").into_boxed_str())
    };
    let first = selector("Text(\"First");
    let second = selector("Text(\"Second");
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));

    let wait_for =
        |vcx: &mut gpui::VisualTestContext, selector, previous_width: Option<gpui::Pixels>| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                vcx.run_until_parked();
                if let Some(bounds) = vcx.debug_bounds(selector)
                    && previous_width.is_none_or(|width| bounds.size.width > width)
                {
                    break bounds;
                }
                assert!(Instant::now() < deadline, "preview did not update");
                std::thread::sleep(Duration::from_millis(15));
            }
        };

    wait_for(vcx, first, None);
    assert!(vcx.debug_bounds("studio-canvas").unwrap().size.width > gpui::px(0.));
    assert!(vcx.debug_bounds(second).is_none());
    let start = source.find("128").unwrap();
    document.state.update(|state| {
        state.selection = start..start + 3;
        state.selection_request = Some(start..start + 3);
    });
    app.focus_editor();
    let before = wait_for(vcx, second, None);
    assert!(vcx.debug_bounds(first).is_none());

    // This goes through the mounted Code input and the background preview task.
    vcx.simulate_input("178");
    assert_eq!(
        document.state.get().text.to_string(),
        source.replace("128", "178")
    );
    wait_for(vcx, second, Some(before.size.width));
    assert!(vcx.debug_bounds(first).is_none());

    let panel = vcx.debug_bounds("studio-floating-panel").unwrap();
    let hit = panel.origin + gpui::point(panel.size.width / 2., gpui::px(78.));
    vcx.simulate_click(hit, Default::default());
    vcx.run_until_parked();
    assert_eq!(
        document.state.get().selection.start,
        source.find("Text(\"Second").unwrap()
    );
    vcx.simulate_keystrokes("backspace");
    let updated = document.state.get().text.to_string();
    assert!(updated.contains("Text(\"First\").frame(80, 40)"));
    assert!(
        !updated.contains("Text(\"Second\")"),
        "{}",
        app.visual_message.get()
    );
}

#[test]
fn marked_grouped_view_opens_in_design_with_its_local_components() {
    let source = r#"
view! {
    HeaderView { Text("Welcome") }

    @Design
    HomeView {
        Layout { HeaderView() ContentView() FooterView() }
    }

    ContentView { Text("Hello from Omen") }
    FooterView { Text("© Omen") }
}
"#;
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-marked-preview-{}", std::process::id())));
    let project = fixture.0.join("project");
    fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let path = project.join("home_view.rs");
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    let header = source.find("Text(\"Welcome\")").unwrap();
    document
        .state
        .update(|state| state.selection = header..header);
    block_on(app.analyze_visual(path, document.state.get().text));

    let labels = |node: &ViewNode, text: &str| {
        let mut matching = Vec::new();
        find_nodes(
            node,
            &|node| matches!(&node.kind, ViewKind::Text(label) if label.starts_with(text)),
            &mut matching,
        );
        matching.len()
    };
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let rendered = runtime.render().unwrap();
    assert_eq!(labels(&rendered, "@Design · HomeView"), 1);
    for label in ["Welcome", "Hello from Omen", "© Omen"] {
        assert_eq!(
            labels(&rendered, label),
            1,
            "{label} should render inside HomeView"
        );
    }

    let content = source.find("Text(\"Hello from Omen\")").unwrap();
    document
        .state
        .update(|state| state.selection = content..content);
    let rendered = runtime.render().unwrap();
    assert_eq!(labels(&rendered, "Hello from Omen"), 1);
    assert_eq!(labels(&rendered, "Welcome"), 0);
    assert_eq!(labels(&rendered, "@Design · HomeView"), 0);
}

#[test]
fn editor_modes_follow_the_live_preview_marker() {
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-preview-mode-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let path = root.join("main.rs");
    let unmarked = r#"// @Design Fake {}
view! { Home { Text("@Design") } }
"#;
    fs::write(&path, unmarked).unwrap();

    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path.clone());
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let has_modes = |root: &ViewNode| {
        let mut modes = Vec::new();
        find_nodes(
            root,
            &|node| matches!(&node.kind, ViewKind::Button { label, .. } if label == "Design" || label == "Split"),
            &mut modes,
        );
        modes.len()
    };
    let has_code = |root: &ViewNode| {
        let mut editors = Vec::new();
        find_nodes(
            root,
            &|node| node.style.code_document.is_some(),
            &mut editors,
        );
        !editors.is_empty()
    };

    app.set_visual_mode(EditorMode::Split);
    assert!(app.workspace.get().canvas(&path).is_none());
    let rendered = runtime.render().unwrap();
    assert_eq!(has_modes(&rendered), 0);
    assert!(has_code(&rendered));

    document.state.update(|state| state.text = SOURCE.into());
    document.emit(EditorEvent::Edited);
    assert_eq!(has_modes(&runtime.render().unwrap()), 2);
    app.set_visual_mode(EditorMode::Split);
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().message, "Ready");
    assert_eq!(
        app.workspace.get().canvas(&path).unwrap().mode,
        EditorMode::Split
    );
    assert!(has_code(&runtime.render().unwrap()));

    document.state.update(|state| {
        let end = state.text.len();
        state.text.replace(end..end, "{");
    });
    assert_eq!(has_modes(&runtime.render().unwrap()), 2);

    let marker = SOURCE.find("@Design").unwrap();
    document.state.update(|state| {
        state.text.replace(marker..marker + "@Design".len(), "");
    });
    app.set_visual_mode(EditorMode::Design);
    assert_eq!(app.canvas_settings(&path).mode, EditorMode::Split);
    let rendered = runtime.render().unwrap();
    assert_eq!(has_modes(&rendered), 0);
    assert!(has_code(&rendered));

    let custom_path = path.with_file_name("screen.omen");
    fs::write(&custom_path, SOURCE).unwrap();
    app.send(Command::OpenFile(custom_path));
    app.flush();
    pool.run_until_stalled();
    assert_eq!(has_modes(&runtime.render().unwrap()), 2);
}

#[test]
fn home_view_previews_in_studio_design_canvas() {
    let source = include_str!("../../website/src/views/home_view.rs");
    let tree = VisualTree::parse(source).unwrap();
    assert_eq!(tree.declaration_name(), Some("Home"));
    assert!(!tree.nodes.is_empty());
    eprintln!("Tree nodes count: {}", tree.nodes.len());
    for (i, node) in tree.nodes.iter().enumerate() {
        if node.component == "AdaptiveGrid" || node.properties.iter().any(|p| p.name == "direction" || p.name == "mode") {
            eprintln!("Node {i}: component={}, props={:?}", node.component, node.properties.iter().map(|p| (&p.name, &p.value)).collect::<Vec<_>>());
        }
    }
}

#[gpui::test]
fn canvas_zoom_preserves_frame_internal_layout_context(cx: &mut gpui::TestAppContext) {
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture = Fixture(std::env::temp_dir().join(format!("omen-test-zoom-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    let source = include_str!("../../website/src/views/home_view.rs");
    fs::write(&home, source).unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();

    // Configure a WebDesktop frame (width: 1280, height: 800)
    app.update_canvas(&home, |canvas| {
        canvas.fit_to_view = false;
        canvas.zoom = 100.;
        canvas.pan = [0., 0.];
        canvas.frames[0] = omen_studio::visual::VisualFrame::new(1, Profile::WebDesktop);
    });
    let text = app.editor_document(home.clone()).state.get().text;
    block_on(app.analyze_visual(home.clone(), text));

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();

    let selector = |text| -> &'static str {
        let offset = source.find(text).unwrap();
        Box::leak(format!("studio-placement-{offset}").into_boxed_str())
    };

    // Headline in hero left column:
    // "Build Native & Web Apps in Rust with SwiftUI-Like Elegance"
    // Card in hero right column:
    // "● ● ●"
    let headline_100 = vcx.debug_bounds(selector("Text(\"Build Native & Web Apps in Rust with SwiftUI-Like Elegance\")"));
    let right_hero_100 = vcx.debug_bounds(selector("Text(\"● ● ●\")"));
    let grid_1_100 = vcx.debug_bounds(selector("Text(\"100%\")"));
    let grid_2_100 = vcx.debug_bounds(selector("Text(\"< 1ms\")"));

    let h100 = headline_100.expect("headline at 100%");
    let r100 = right_hero_100.expect("right hero at 100%");
    let g1_100 = grid_1_100.expect("grid 1 at 100%");
    let g2_100 = grid_2_100.expect("grid 2 at 100%");

    // Now zoom out significantly to 20%
    app.update_canvas(&home, |canvas| {
        canvas.zoom = 20.;
    });
    vcx.run_until_parked();

    let headline_20 = vcx.debug_bounds(selector("Text(\"Build Native & Web Apps in Rust with SwiftUI-Like Elegance\")"));
    let right_hero_20 = vcx.debug_bounds(selector("Text(\"● ● ●\")"));
    let grid_1_20 = vcx.debug_bounds(selector("Text(\"100%\")"));
    let grid_2_20 = vcx.debug_bounds(selector("Text(\"< 1ms\")"));

    let h20 = headline_20.expect("headline at 20%");
    let r20 = right_hero_20.expect("right hero at 20%");
    let g1_20 = grid_1_20.expect("grid 1 at 20%");
    let g2_20 = grid_2_20.expect("grid 2 at 20%");

    // Verify layout topology: hero remains side-by-side (headline is to the left of right_hero)
    // rather than collapsing into single-column layout
    assert!(h100.origin.x < r100.origin.x, "headline should be left of right_hero at 100%");
    assert!(h20.origin.x < r20.origin.x, "headline should remain left of right_hero at 20%");

    // Verify adaptive grid items remain on the same row (same y) at both 100% and 20%
    assert_eq!(g1_100.origin.y, g2_100.origin.y, "grid items should align vertically at 100%");
    assert_eq!(g1_20.origin.y, g2_20.origin.y, "grid items should align vertically at 20%");

    // Verify visual metrics scaled down proportionally to ~0.20
    let headline_ratio = f32::from(h20.size.width) / f32::from(h100.size.width);
    assert!((headline_ratio - 0.20).abs() < 0.02, "expected headline width ratio ~0.20, got {headline_ratio}");

    let grid_ratio = f32::from(g1_20.size.width) / f32::from(g1_100.size.width);
    assert!((grid_ratio - 0.20).abs() < 0.02, "expected grid width ratio ~0.20, got {grid_ratio}");
}

#[gpui::test]
fn frame_focus_eye_disables_design_surface_and_routes_scroll_and_mouse_to_frame(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::{MouseButton, ScrollDelta, ScrollEvent, point, px};
    use omen_gpui::OmenWindow;

    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-frame-focus-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home_view.rs");
    let rows = (1..=30)
        .map(|i| format!("            Text(\"Row {i}\").frame(.infinity, 40)\n"))
        .collect::<String>();
    let source = format!(
        r#"use omen::prelude::*;
view! {{
    @Design
    pub Home {{
        Layout {{
            TextField("Note")
            Button("Open details") {{ Route::Detail.push(); }}
{rows}            TextField("Bottom note")
        }}.spacing(4)
    }}
    pub Detail {{
        Text("Detail page")
    }}
}}
"#
    );
    fs::write(&home, &source).unwrap();

    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(home.clone()));
    app.flush();
    pool.run_until_stalled();

    app.update_canvas(&home, |canvas| {
        canvas.fit_to_view = false;
        canvas.zoom = 100.;
        canvas.pan = [0., 0.];
        canvas.frames[0] = omen_studio::visual::VisualFrame::new(1, Profile::IPhone);
        canvas.frames[0].height = 300;
    });
    let document = app.editor_document(home.clone());
    block_on(app.analyze_visual(home.clone(), document.state.get().text));

    let has_icon = |root: &ViewNode, icon_name: &str| -> bool {
        let mut found = Vec::new();
        find_nodes(
            root,
            &|node| matches!(&node.kind, ViewKind::Icon(name) if name == icon_name),
            &mut found,
        );
        !found.is_empty()
    };

    let mut runtime = Runtime::new(app.editor_view().into_node());
    let before_view = runtime.render().unwrap();
    assert!(has_icon(&before_view, "eye-closed"));
    assert!(!has_icon(&before_view, "eye"));

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();

    let surface = vcx.debug_bounds("studio-canvas").unwrap();
    let grip = vcx.debug_bounds("studio-frame-grip").unwrap();
    let screen = vcx.debug_bounds("studio-device-screen").unwrap();
    assert!(vcx.debug_bounds("studio-floating-panel").is_some());

    // Eye button sits immediately to the right of the "⋯" settings button (width 23 + spacing 5).
    let eye_button = point(grip.right() + px(44.5), grip.center().y);
    assert!(surface.contains(&eye_button));
    vcx.simulate_click(eye_button, Default::default());
    vcx.run_until_parked();

    let focused_settings = app.canvas_settings(&home);
    assert_eq!(focused_settings.focused_frame, Some(1));
    let focused_view = runtime.render().unwrap();
    assert!(has_icon(&focused_view, "eye"));
    assert!(!has_icon(&focused_view, "eye-closed"));
    assert!(vcx.debug_bounds("studio-frame-grip").is_none());
    assert!(vcx.debug_bounds("studio-floating-panel").is_none());

    let zoom_before = focused_settings.zoom;
    let pan_before = focused_settings.pan;
    let pos_before = focused_settings.frames[0].position;

    // Scrolling with Ctrl over the frame must NOT zoom the design section.
    vcx.simulate_event(ScrollEvent {
        position: screen.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(60.))),
        modifiers: gpui::Modifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    });
    vcx.run_until_parked();
    assert_eq!(app.canvas_settings(&home).zoom, zoom_before);
    assert_eq!(app.canvas_settings(&home).pan, pan_before);

    // Dragging on the canvas background or frame header must NOT pan or move the frame.
    let bg = surface.origin + point(px(12.), px(12.));
    vcx.simulate_mouse_down(bg, MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(bg + point(px(40.), px(30.)), MouseButton::Left, Default::default());
    vcx.simulate_mouse_up(bg + point(px(40.), px(30.)), MouseButton::Left, Default::default());
    vcx.simulate_mouse_down(grip.center(), MouseButton::Left, Default::default());
    vcx.simulate_mouse_move(
        grip.center() + point(px(30.), px(20.)),
        MouseButton::Left,
        Default::default(),
    );
    vcx.simulate_mouse_up(
        grip.center() + point(px(30.), px(20.)),
        MouseButton::Left,
        Default::default(),
    );
    vcx.run_until_parked();
    assert_eq!(app.canvas_settings(&home).pan, pan_before);
    assert_eq!(app.canvas_settings(&home).frames[0].position, pos_before);

    // Interactive controls inside the focused frame receive mouse clicks and keyboard input
    // without triggering design selection or design backspace deletion.
    let initial_selection = document.state.get().selection.clone();
    vcx.simulate_click(screen.origin + point(px(60.), px(14.)), Default::default());
    vcx.run_until_parked();
    let note_focus = vcx.update(|window, cx| window.focused(cx));
    assert!(note_focus.is_some(), "top TextField must accept focus");
    vcx.simulate_input("ab");
    vcx.simulate_keystrokes("backspace");
    vcx.run_until_parked();
    assert_eq!(document.state.get().text.to_string(), source);
    assert_eq!(document.state.get().selection, initial_selection);

    // Clicking the button navigates to Detail rather than selecting code.
    vcx.simulate_click(screen.origin + point(px(60.), px(46.)), Default::default());
    vcx.run_until_parked();
    assert_eq!(document.state.get().selection, initial_selection);

    // Click Back in the app bar to return to the scrollable Home view, then scroll the frame.
    vcx.simulate_click(screen.origin + point(px(24.), px(24.)), Default::default());
    vcx.run_until_parked();

    // Before scrolling, the bottom TextField is offscreen, so clicking near the bottom hits a Text row.
    vcx.simulate_click(
        screen.origin + point(px(60.), px(280.)),
        Default::default(),
    );
    vcx.run_until_parked();
    let pre_scroll_focus = vcx.update(|window, cx| window.focused(cx));
    assert_ne!(pre_scroll_focus, note_focus);

    // Scroll down inside the frame: Bottom note becomes visible and focusable at the bottom of screen,
    // while canvas pan and zoom remain untouched.
    for _ in 0..10 {
        vcx.simulate_event(ScrollEvent {
            position: screen.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-240.))),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
    }
    vcx.run_until_parked();
    assert_eq!(app.canvas_settings(&home).pan, pan_before);
    assert_eq!(app.canvas_settings(&home).zoom, zoom_before);
    vcx.simulate_click(
        screen.origin + point(px(60.), px(280.)),
        Default::default(),
    );
    vcx.run_until_parked();
    let bottom_focus = vcx.update(|window, cx| window.focused(cx));
    assert!(
        bottom_focus.is_some() && bottom_focus != pre_scroll_focus && bottom_focus != note_focus,
        "scrolling the focused frame must bring the bottom TextField into view"
    );

    // Clicking the open eye button turns focus off and restores design surface controls.
    vcx.simulate_click(eye_button, Default::default());
    vcx.run_until_parked();
    assert_eq!(app.canvas_settings(&home).focused_frame, None);
    let unfocused_view = runtime.render().unwrap();
    assert!(has_icon(&unfocused_view, "eye-closed"));
    assert!(!has_icon(&unfocused_view, "eye"));
    assert!(vcx.debug_bounds("studio-frame-grip").is_some());

    // Design surface Ctrl+scroll now zooms again.
    vcx.simulate_event(ScrollEvent {
        position: screen.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(36.))),
        modifiers: gpui::Modifiers {
            control: true,
            ..Default::default()
        },
        ..Default::default()
    });
    vcx.run_until_parked();
    assert!(app.canvas_settings(&home).zoom > zoom_before);
}


