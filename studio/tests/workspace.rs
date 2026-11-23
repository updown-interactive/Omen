use futures::{StreamExt, executor::block_on};
use omen_studio::{
    app::StudioEnvironment,
    services,
    state::WorkspaceState,
    theme::LayoutPreferences,
    workspace::{
        Command, Workspace,
        service::{Event, WorkspaceService},
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omen-studio-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("project/src")).unwrap();
        fs::write(
            path.join("project/src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        Self(path)
    }
    fn project(&self) -> PathBuf {
        self.0.join("project").canonicalize().unwrap()
    }
    fn file(&self) -> PathBuf {
        self.project().join("src/main.rs")
    }
    fn workspace(&self) -> Workspace {
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&self.project()).unwrap();
        workspace.open_file(&self.file()).unwrap();
        workspace
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn edit_save_and_reopen_preserve_unicode_and_newlines() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let content = "// 👩‍💻\nfn main() {}\n".to_string();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: content.clone(),
        })
        .unwrap();
    assert_eq!(workspace.state.unsaved_count(), 1);
    workspace.save_file(&fixture.file()).unwrap();
    let reopened = fixture.workspace();
    assert_eq!(reopened.state.active_document().unwrap().content, content);
    assert_eq!(workspace.state.unsaved_count(), 0);
}

#[test]
fn external_changes_reload_clean_documents_and_protect_dirty_documents() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    fs::write(fixture.file(), "external clean\n").unwrap();
    assert!(workspace.poll_external().unwrap());
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "external clean\n"
    );
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "my edits\n".into(),
        })
        .unwrap();
    fs::write(fixture.file(), "another process\n").unwrap();
    assert!(workspace.save_file(&fixture.file()).is_err());
    assert!(workspace.state.active_document().unwrap().conflict);
    assert_eq!(
        fs::read_to_string(fixture.file()).unwrap(),
        "another process\n"
    );
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "my edits\n"
    );
}

#[test]
fn dirty_documents_cannot_be_closed_or_lost_by_switching_projects() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "unsaved".into(),
        })
        .unwrap();
    let other = fixture.0.join("other");
    fs::create_dir(&other).unwrap();
    assert!(workspace.apply(Command::Close(fixture.file())).is_err());
    assert!(workspace.open(&other).is_err());
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "unsaved"
    );
}

#[test]
fn tree_expands_lazily_and_skips_build_directories() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.project().join("target")).unwrap();
    let mut workspace = Workspace::new(WorkspaceState::default());
    workspace.open(&fixture.project()).unwrap();
    assert_eq!(workspace.state.rows.len(), 1);
    assert!(workspace.state.rows[0].directory);
    workspace
        .apply(Command::ToggleDirectory(fixture.project().join("src")))
        .unwrap();
    assert!(
        workspace
            .state
            .rows
            .iter()
            .any(|row| row.path == fixture.file() && row.depth == 1)
    );
    workspace
        .apply(Command::ToggleDirectory(fixture.project().join("src")))
        .unwrap();
    assert_eq!(workspace.state.rows.len(), 1);
}

#[test]
fn binary_large_and_outside_files_are_rejected() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let binary = fixture.project().join("binary");
    fs::write(&binary, [0, 1, 2]).unwrap();
    assert!(workspace.open_file(&binary).is_err());
    let large = fixture.project().join("large");
    fs::File::create(&large)
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    assert!(workspace.open_file(&large).is_err());
    let outside = fixture.0.join("outside.rs");
    fs::write(&outside, "private").unwrap();
    assert!(workspace.open_file(&outside).is_err());
}

fn wait_for(
    updates: &mut futures::channel::mpsc::UnboundedReceiver<Event>,
    sequence: u64,
) -> WorkspaceState {
    loop {
        let event = block_on(updates.next()).unwrap();
        let state = match event {
            Event::Loaded(_, _, _, state) | Event::Changed(state) => state,
            Event::AccountSaved(_) => continue,
        };
        if state.sequence >= sequence {
            return state;
        }
    }
}

#[test]
fn restart_restores_unsaved_documents_tabs_and_separate_layout_preferences() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("settings");
    let (service, mut updates) = WorkspaceService::start(directory.clone()).unwrap();
    wait_for(&mut updates, 0);
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    service.send(2, Command::OpenFile(fixture.file())).unwrap();
    service
        .send(
            3,
            Command::Edit {
                path: fixture.file(),
                content: "unsaved after restart\n".into(),
            },
        )
        .unwrap();
    let layout = LayoutPreferences {
        sidebar_visible: false,
        preview_width: 410.,
        sidebar_width: 320.,
        preview_visible: false,
        active_sidebar: Default::default(),
    };
    service.layout(layout.clone()).unwrap();
    service.flush().unwrap();
    assert_eq!(wait_for(&mut updates, 3).unsaved_count(), 1);
    let restored: WorkspaceState = services::read_json(&directory.join("workspace.json")).unwrap();
    assert_eq!(
        restored.active_document().unwrap().content,
        "unsaved after restart\n"
    );
    assert_eq!(restored.recent, [fixture.project()]);
    let restored_layout: LayoutPreferences =
        services::read_json(&directory.join("layout.json")).unwrap();
    assert_eq!(restored_layout, layout);
    drop(service);
    drop(updates);
    let (restarted, mut events) = WorkspaceService::start(directory.clone()).unwrap();
    let Event::Loaded(recovered_layout, _, _, recovered) = block_on(events.next()).unwrap() else {
        panic!("expected initial restoration");
    };
    assert_eq!(recovered_layout, layout);
    assert_eq!(recovered.active_file, Some(fixture.file()));
    assert_eq!(
        recovered.active_document().unwrap().content,
        "unsaved after restart\n"
    );
    // Session backups must not modify the actual project file.
    assert!(
        fs::read_to_string(fixture.file())
            .unwrap()
            .starts_with("fn main()")
    );
    restarted.flush().unwrap();
    drop(restarted);
    drop(events);
}

#[test]
fn studio_creation_uses_the_cli_generator_and_never_overwrites_a_project() {
    let fixture = Fixture::new();
    let mut workspace = Workspace::new(WorkspaceState::default());
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let command = Command::Create {
        name: "my-omen-app".into(),
        parent: fixture.0.clone(),
        sdk,
    };
    workspace.apply(command.clone()).unwrap();
    let root = fixture.0.join("my-omen-app").canonicalize().unwrap();
    assert_eq!(workspace.state.root, Some(root.clone()));
    assert!(
        fs::read_to_string(root.join("Cargo.toml"))
            .unwrap()
            .contains("omen =")
    );
    fs::write(root.join("src/main.rs"), "keep my source").unwrap();
    assert!(workspace.apply(command).is_err());
    assert_eq!(
        fs::read_to_string(root.join("src/main.rs")).unwrap(),
        "keep my source"
    );
}

#[test]
fn queued_older_events_do_not_overwrite_newer_typing() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    pool.run_until_stalled();
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    let binding = app.content(fixture.file());
    binding.set("first".into());
    binding.set("second".into());
    binding.set("third 👋\n".into());
    app.flush();
    pool.run_until_stalled();
    assert_eq!(binding.get(), "third 👋\n");
}

#[test]
fn declarative_shell_opens_tabs_edits_and_persists_visibility() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, View, ViewKind, ViewNode};
    fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(label)
            || matches!(&node.kind, ViewKind::Text(text)|ViewKind::Button{label:text,..}|ViewKind::TextField{label:text,..} if text==label)
        {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| find(child, label))
            }
            _ => None,
        }
    }
    let fixture = Fixture::new();
    let settings = fixture.0.join("settings");
    let app = StudioEnvironment::at(settings.clone());
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(app.editor_view().into_node());
    assert!(find(&runtime.render().unwrap(), "No open documents").is_some());
    app.send(Command::OpenProject(fixture.project()));
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    let manifest = fixture.project().join("Cargo.toml");
    fs::write(&manifest, "[package]\nname = \"example\"\n").unwrap();
    app.send(Command::OpenFile(manifest.clone()));
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file, Some(manifest));
    let root = runtime.render().unwrap();
    if let ViewKind::Button { action, .. } = &find(&root, "main.rs").unwrap().kind {
        action.invoke();
        action.invoke();
    }
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file, Some(fixture.file()));
    let root = runtime.render().unwrap();
    let editor = find(&root, "Code editor").unwrap();
    assert!(editor.style.multiline);
    let code = editor.style.code_document.as_ref().unwrap();
    code.state
        .update(|state| state.text = "// edited through Omen binding\nfn main() {}\n".into());
    code.emit(omen::EditorEvent::Edited);
    app.flush();
    pool.run_until_stalled();
    let root = runtime.render().unwrap();
    assert!(find(&root, "main.rs •").is_some());
    if let ViewKind::Button { action, .. } = &find(&root, "File").unwrap().kind {
        action.invoke();
    }
    if let ViewKind::Button { action, .. } = &find(&runtime.render().unwrap(), "Save").unwrap().kind
    {
        action.invoke();
    }
    app.flush();
    pool.run_until_stalled();
    assert!(
        fs::read_to_string(fixture.file())
            .unwrap()
            .starts_with("// edited through Omen binding\n")
    );
    if let ViewKind::Button { action, .. } =
        &find(&runtime.render().unwrap(), "Window").unwrap().kind
    {
        action.invoke();
    }
    if let ViewKind::Button { action, .. } = &find(&runtime.render().unwrap(), "Toggle sidebar")
        .unwrap()
        .kind
    {
        action.invoke();
    }
    app.width(false).set(380.);
    app.flush();
    let preferences: LayoutPreferences =
        services::read_json(&settings.join("layout.json")).unwrap();
    assert!(!preferences.sidebar_visible);
    assert_eq!(preferences.preview_width, 380.);
    assert!(find(&runtime.render().unwrap(), "Explorer").is_none());
    app.update_layout(|layout| {
        layout.sidebar_visible = true;
        layout.preview_visible = true;
    });
    app.width(true).set(480.);
    app.width(false).set(600.);
    runtime.set_viewport(omen::Viewport {
        width: 1000.,
        height: 700.,
        ..Default::default()
    });
    fn panel_width(node: &ViewNode, key: &str) -> Option<f32> {
        if node.style.key.as_deref() == Some(key) {
            return node.style.frame.map(|frame| frame.width);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| panel_width(child, key))
            }
            _ => None,
        }
    }
    let narrow = runtime.render().unwrap();
    assert!(
        panel_width(&narrow, "sidebar-pane").unwrap()
            + panel_width(&narrow, "preview-pane").unwrap()
            <= 1000. - 320. - 12.
    );
    assert_eq!(
        app.layout.get().preview_width,
        600.,
        "temporary viewport constraints preserve preferred sizes"
    );
}

#[test]
fn mounted_workspace_updates_resume_after_background_cancellation() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{AppLifecycle, Runtime, Value, View, ViewExt};
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    let mut pool = LocalPool::new();
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    runtime.render().unwrap();
    for job in runtime.take_tasks() {
        pool.spawner().spawn_local(job.run()).unwrap();
    }
    app.flush();
    pool.run_until_stalled();
    assert!(app.ready.get());
    runtime.set_lifecycle(AppLifecycle::Background);
    runtime.render().unwrap();
    // Queue an event while the old mounted task is awaiting stream input, then
    // remount before polling its cancellation. The new receiver must wait safely.
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    runtime.set_lifecycle(AppLifecycle::Foreground);
    runtime.render().unwrap();
    for job in runtime.take_tasks() {
        pool.spawner().spawn_local(job.run()).unwrap();
    }
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().root, Some(fixture.project()));
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file, Some(fixture.file()));
}

#[test]
fn settings_sidebar_updates_the_editor_and_persists_preferences_separately_from_layout() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, View, ViewKind, ViewNode};
    use omen_studio::{
        theme::ThemePreset,
        visual::{EditorMode, Profile},
    };
    fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(label)
            || matches!(&node.kind, ViewKind::Button { label: text, .. } | ViewKind::Checkbox { label: text, .. } | ViewKind::TextField { label: text, .. } if text == label)
        {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(c)
            | ViewKind::Layout(c)
            | ViewKind::List(c)
            | ViewKind::Card(c)
            | ViewKind::Presentation { children: c, .. } => {
                c.iter().find_map(|child| find(child, label))
            }
            _ => None,
        }
    }
    fn click(node: &ViewNode, label: &str) {
        let target = find(node, label).unwrap();
        if let Some(action) = &target.style.on_tap {
            action.invoke();
        } else if let ViewKind::Button { action, .. } = &target.kind {
            action.invoke();
        } else {
            panic!("not a button");
        }
    }
    let fixture = Fixture::new();
    let settings = fixture.0.join("appearance");
    let app = StudioEnvironment::at(settings.clone());
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(app.editor_view().into_node());
    fn toolbar(node: &ViewNode) -> Option<&ViewNode> {
        if node.style.desktop_toolbar.is_some() {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children) | ViewKind::Layout(children) => {
                children.iter().find_map(toolbar)
            }
            _ => None,
        }
    }
    let initial = runtime.render().unwrap();
    assert!(
        find(
            toolbar(&initial).expect("native title-bar toolbar"),
            "Settings"
        )
        .is_some()
    );
    if let ViewKind::Layout(children) = &initial.kind {
        assert!(
            find(children.last().unwrap(), "Settings").is_none(),
            "settings only appears in the native toolbar"
        );
    }
    click(&runtime.render().unwrap(), "Settings");
    let rendered = runtime.render().unwrap();
    assert!(find(&rendered, "Select Default theme").is_some());
    assert!(find(&rendered, "Settings sections").is_some());
    assert_eq!(
        find(&rendered, "Appearance")
            .unwrap()
            .style
            .accessibility
            .selected,
        Some(true)
    );
    click(&runtime.render().unwrap(), "Select Midnight theme");
    assert_eq!(app.settings.get().appearance.theme, ThemePreset::Midnight);
    assert_eq!(app.colors(), ThemePreset::Midnight.colors(true));
    click(&runtime.render().unwrap(), "Use light appearance");
    assert_eq!(app.colors(), ThemePreset::Midnight.colors(false));
    click(&runtime.render().unwrap(), "Select Graphite theme");
    assert_eq!(app.colors(), ThemePreset::Graphite.colors(false));
    click(&runtime.render().unwrap(), "Editor");
    let rendered = runtime.render().unwrap();
    assert_eq!(
        find(&rendered, "Editor")
            .unwrap()
            .style
            .accessibility
            .selected,
        Some(true)
    );
    assert!(find(&runtime.render().unwrap(), "Select Graphite theme").is_none());
    click(&runtime.render().unwrap(), "Auto Save: After Delay");
    click(&runtime.render().unwrap(), "Increase auto save delay");
    click(&runtime.render().unwrap(), "Increase font size");
    click(&runtime.render().unwrap(), "Increase line height");
    click(&runtime.render().unwrap(), "Increase letter spacing");

    click(&runtime.render().unwrap(), "Design");
    assert!(find(&runtime.render().unwrap(), "Increase font size").is_none());
    click(
        &runtime.render().unwrap(),
        "Use Split as the default editor mode",
    );
    click(
        &runtime.render().unwrap(),
        "Use Desktop browser as the default frame",
    );
    let rendered = runtime.render().unwrap();
    let value = find(&rendered, "Show canvas grid")
        .and_then(|node| node.style.checked.as_ref())
        .expect("Expected an application-owned grid binding");
    assert!(value.get());
    value.set(false);

    click(&runtime.render().unwrap(), "Account");
    let rendered = runtime.render().unwrap();
    for (label, value) in [
        ("Full name", "Ada Lovelace"),
        ("Email address", "ada@example.test"),
    ] {
        find(&rendered, label)
            .unwrap()
            .style
            .text_value
            .as_ref()
            .unwrap()
            .set(value.into());
    }
    click(&runtime.render().unwrap(), "Save Account");
    app.flush();
    pool.run_until_stalled();
    let saved_account: omen_studio::account::Account =
        services::read_json(&settings.join("account.json")).unwrap();
    assert_eq!(saved_account.name, "Ada Lovelace");
    assert_eq!(saved_account.email, "ada@example.test");

    click(&runtime.render().unwrap(), "Done");
    let project = fixture.project();
    let view = project.join("src/home_view.rs");
    fs::write(&view, r#"view! { @Design Home { Text("Home") } }"#).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(view.clone()));
    app.flush();
    pool.run_until_stalled();
    let canvas = app.canvas_settings(&view);
    assert_eq!(canvas.mode, EditorMode::Split);
    assert_eq!(canvas.frames[0].profile, Profile::WebDesktop);
    let document = app.editor_document(view.clone());
    assert_eq!(document.state.get().letter_spacing, 1.);
    block_on(app.analyze_visual(view, document.state.get().text));

    fn find_style(
        node: &ViewNode,
        predicate: &impl Fn(&omen::Style) -> bool,
    ) -> Option<omen::Style> {
        if predicate(&node.style) {
            return Some(node.style.clone());
        }
        match &node.kind {
            ViewKind::Group(children) | ViewKind::Layout(children) | ViewKind::List(children) => {
                children
                    .iter()
                    .find_map(|child| find_style(child, predicate))
            }
            _ => None,
        }
    }
    let rendered = runtime.render().unwrap();
    let editor = find_style(&rendered, &|style| style.code_document.is_some()).unwrap();
    assert_eq!(editor.font_size, Some(14.));
    assert_eq!(editor.line_height, Some(21.));
    let surface = find_style(&rendered, &|style| {
        style
            .preview
            .as_ref()
            .is_some_and(|preview| preview.navigation.is_some())
    })
    .unwrap();
    assert!(surface.preview.unwrap().grid.is_none());

    let saved: omen_studio::settings::StudioSettings =
        services::read_json(&settings.join("settings.json")).unwrap();
    assert_eq!(saved.appearance.theme, ThemePreset::Graphite);
    assert_eq!(
        saved.appearance.mode,
        omen_studio::settings::AppearanceMode::Light
    );
    assert_eq!(saved.theme().resolve(false).canvas, 0xfdfcfb);
    let layout: serde_json::Value = services::read_json(&settings.join("layout.json")).unwrap();
    assert!(layout.get("preset").is_none());
    assert!(layout.get("dark").is_none());
    assert_eq!(
        saved.editor.auto_save,
        omen_studio::settings::AutoSaveMode::AfterDelay
    );
    assert_eq!(saved.editor.auto_save_delay_ms, 1250);
    assert_eq!(saved.editor.font_size, 14);
    assert_eq!(saved.editor.line_height, 21);
    assert_eq!(saved.editor.letter_spacing, 1.);
    assert_eq!(saved.design.initial_mode, EditorMode::Split);
    assert_eq!(saved.design.frame_profile, Profile::WebDesktop);
    assert!(!saved.design.show_grid);
    let restarted = StudioEnvironment::at(settings);
    pool.spawner().spawn_local(restarted.receive()).unwrap();
    restarted.flush();
    pool.run_until_stalled();
    assert_eq!(restarted.settings.get(), saved);
    assert_eq!(restarted.account.get(), saved_account);
}

#[test]
fn resizing_a_constrained_panel_starts_at_its_visible_width() {
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("resize"));
    app.ready.set(true);
    app.update_layout(|layout| {
        layout.sidebar_width = 480.;
        layout.preview_width = 600.;
    });
    let displayed = app.layout.get().geometry(1200.);
    let handle = app.visible_width(false, 1200.);
    assert_eq!(handle.get(), displayed.preview_width);
    handle.set(handle.get() - 10.);
    assert_eq!(
        app.layout.get().preview_width,
        (displayed.preview_width - 10.).max(280.)
    );
}

#[test]
fn a_watcher_reload_cannot_authorize_overwriting_a_queued_editor_snapshot() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let displayed = workspace.state.active_document().unwrap().clone();
    fs::write(fixture.file(), "external change\n").unwrap();
    workspace.poll_external().unwrap();
    let result = workspace.apply(Command::EditFromSnapshot {
        path: fixture.file(),
        content: "my queued edit\n".into(),
        disk_revision: displayed.disk_revision,
        baseline: displayed.baseline.to_string(),
    });
    assert!(result.is_err());
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "my queued edit\n"
    );
    assert!(workspace.save_file(&fixture.file()).is_err());
    assert_eq!(
        fs::read_to_string(fixture.file()).unwrap(),
        "external change\n"
    );
}

#[test]
fn continued_typing_after_an_async_save_does_not_create_a_false_external_conflict() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let displayed = workspace.state.active_document().unwrap().clone();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "saved first\n".into(),
        })
        .unwrap();
    workspace.save_file(&fixture.file()).unwrap();
    workspace
        .apply(Command::EditFromSnapshot {
            path: fixture.file(),
            content: "continued typing\n".into(),
            disk_revision: displayed.disk_revision,
            baseline: displayed.baseline.to_string(),
        })
        .unwrap();
    workspace.save_file(&fixture.file()).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.file()).unwrap(),
        "continued typing\n"
    );
}

#[test]
fn service_shutdown_drains_commands_even_when_the_ui_event_stream_is_dropped() {
    let fixture = Fixture::new();
    let profile = fixture.0.join("shutdown");
    let (service, events) = WorkspaceService::start(profile.clone()).unwrap();
    drop(events);
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    service.send(2, Command::OpenFile(fixture.file())).unwrap();
    service
        .send(
            3,
            Command::Edit {
                path: fixture.file(),
                content: "recover this buffer\n".into(),
            },
        )
        .unwrap();
    service
        .layout(LayoutPreferences {
            sidebar_visible: false,
            preview_visible: false,
            sidebar_width: 310.,
            preview_width: 390.,
            active_sidebar: Default::default(),
        })
        .unwrap();
    drop(service);
    let restored: WorkspaceState = services::read_json(&profile.join("workspace.json")).unwrap();
    assert_eq!(
        restored.active_document().unwrap().content,
        "recover this buffer\n"
    );
    let layout: LayoutPreferences = services::read_json(&profile.join("layout.json")).unwrap();
    assert!(!layout.sidebar_visible && !layout.preview_visible);
    assert_eq!(layout.sidebar_width, 310.);
    assert_eq!(layout.preview_width, 390.);
}

#[test]
fn service_reads_files_asynchronously_without_a_ui_subscription() {
    let fixture = Fixture::new();
    let (service, events) = WorkspaceService::start(fixture.0.join("read")).unwrap();
    drop(events);
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    let content = block_on(service.read_file(fixture.file())).unwrap();
    assert!(content.contains("fn main()"));
    assert!(block_on(service.read_file(fixture.0.join("outside"))).is_err());
}

#[test]
fn restoring_the_expected_disk_contents_clears_a_transient_conflict() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let baseline = workspace.state.active_document().unwrap().baseline.clone();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "local edit\n".into(),
        })
        .unwrap();
    fs::write(fixture.file(), "external edit\n").unwrap();
    workspace.poll_external().unwrap();
    assert!(workspace.state.active_document().unwrap().conflict);
    fs::write(fixture.file(), baseline.to_string()).unwrap();
    workspace.poll_external().unwrap();
    assert!(!workspace.state.active_document().unwrap().conflict);
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "local edit\n"
    );
}

#[test]
fn configured_project_uses_manifest_and_targets_before_generation() {
    use omen_cli::project::{manifest::TargetPlatform, template};
    let fixture = Fixture::new();
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut manifest = template::default_manifest("configured", Some(sdk.clone()));
    manifest.app.name = "My Application".into();
    manifest.app.id = "org.example.custom".into();
    manifest.window.width = 980;
    manifest.targets.platforms = vec![TargetPlatform::Web];
    let source = toml::to_string_pretty(&manifest).unwrap();
    let mut workspace = Workspace::new(WorkspaceState::default());
    workspace
        .apply(Command::CreateConfigured {
            name: "configured".into(),
            parent: fixture.0.clone(),
            sdk,
            source,
        })
        .unwrap();
    let root = fixture.0.join("configured");
    let saved: omen_cli::project::manifest::Manifest =
        toml::from_str(&fs::read_to_string(root.join("Omen.toml")).unwrap()).unwrap();
    assert_eq!(saved.app.id, "org.example.custom");
    assert_eq!(saved.window.width, 980);
    assert_eq!(saved.targets.platforms, vec![TargetPlatform::Web]);
    assert!(saved.require_target("desktop").is_err());
    assert!(
        fs::read_to_string(root.join("Cargo.toml"))
            .unwrap()
            .contains("default = [\"web\"]")
    );
    assert_eq!(workspace.state.root, Some(root.canonicalize().unwrap()));
}

#[test]
fn project_dialog_validates_before_creating_files_and_preserves_invalid_text() {
    use omen_cli::project::manifest::{Manifest, TargetPlatform};
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    app.open_path.set(fixture.0.to_string_lossy().into());
    app.project_name.set("wizard-app".into());
    app.create();
    assert!(app.creation_visible.get());
    assert!(!fixture.0.join("wizard-app").exists());
    app.target(TargetPlatform::Web).set(true);
    app.target(TargetPlatform::Desktop).set(false);
    let manifest: Manifest = toml::from_str(&app.manifest_source.get()).unwrap();
    assert_eq!(manifest.targets.platforms, vec![TargetPlatform::Web]);
    app.manifest_source.set("invalid = [".into());
    app.submit_create();
    assert!(!app.creation_error.get().is_empty());
    assert_eq!(app.manifest_source.get(), "invalid = [");
    assert!(!fixture.0.join("wizard-app").exists());
}

#[test]
fn manifest_target_validation_and_legacy_defaults() {
    use omen_cli::project::{manifest::Manifest, template};
    let mut manifest = template::default_manifest("targets", None);
    let mut legacy = toml::Value::try_from(&manifest).unwrap();
    legacy.as_table_mut().unwrap().remove("targets");
    let legacy: Manifest = legacy.try_into().unwrap();
    assert_eq!(legacy.targets.platforms.len(), 4);
    manifest.targets.platforms.clear();
    assert!(manifest.validate().is_err());
    manifest.targets.platforms = vec![omen_cli::project::manifest::TargetPlatform::Desktop; 2];
    assert!(manifest.validate().is_err());
}

#[test]
fn creation_dialog_renders_and_success_closes_after_worker_completion() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, Value, View, ViewExt};
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    let mut pool = LocalPool::new();
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    runtime.render().unwrap();
    for job in runtime.take_tasks() {
        pool.spawner().spawn_local(job.run()).unwrap();
    }
    app.flush();
    pool.run_until_stalled();
    app.open_path.set(fixture.0.to_string_lossy().into());
    app.project_name.set("dialog-project".into());
    app.create();
    runtime.render().unwrap();
    assert!(app.creation_visible.get());
    app.submit_create();
    app.flush();
    pool.run_until_stalled();
    assert!(!app.creation_visible.get(), "{}", app.creation_error.get());
    assert_eq!(
        app.workspace.get().root,
        Some(fixture.0.join("dialog-project").canonicalize().unwrap())
    );
    runtime.render().unwrap();
}

#[test]
fn configured_generator_rejects_invalid_targets_without_creating_a_directory() {
    use omen_cli::project::{manifest::TargetPlatform, template};
    let fixture = Fixture::new();
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut manifest = template::default_manifest("invalid-targets", Some(sdk.clone()));
    manifest.targets.platforms.clear();
    let source = toml::to_string_pretty(&manifest).unwrap();
    assert!(
        template::create_from_toml("invalid-targets", &fixture.0, Some(&sdk), &source).is_err()
    );
    assert!(!fixture.0.join("invalid-targets").exists());
    manifest.targets.platforms.push(TargetPlatform::Desktop);
    let source = toml::to_string_pretty(&manifest).unwrap();
    let unknown = source.replace("desktop", "unknown");
    assert!(
        template::create_from_toml("invalid-targets", &fixture.0, Some(&sdk), &unknown).is_err()
    );
    assert!(!fixture.0.join("invalid-targets").exists());
}

#[test]
fn unavailable_recent_projects_are_pruned_and_missing_root_opens_a_clean_home() {
    let fixture = Fixture::new();
    let missing = fixture.0.join("deleted-project");
    let mut state = WorkspaceState {
        root: Some(missing.clone()),
        recent: vec![missing.clone(), fixture.project()],
        ..Default::default()
    };
    state.expanded.insert(missing);
    let profile = fixture.0.join("settings");
    services::write_json(&profile.join("workspace.json"), &state).unwrap();
    let (service, mut events) = WorkspaceService::start(profile.clone()).unwrap();
    let restored = match block_on(events.next()).unwrap() {
        Event::Loaded(_, _, _, state) => state,
        _ => panic!("Expected restored state"),
    };
    assert!(restored.root.is_none());
    assert!(restored.rows.is_empty());
    assert!(restored.expanded.is_empty());
    assert_eq!(restored.recent, vec![fixture.project()]);
    assert_eq!(restored.message, "Ready");
    service.flush().unwrap();
    let saved: WorkspaceState = services::read_json(&profile.join("workspace.json")).unwrap();
    assert_eq!(saved.recent, vec![fixture.project()]);
    assert!(saved.root.is_none());
}

#[test]
fn missing_root_recovery_keeps_unsaved_document_text() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "unsaved recovery text".into(),
        })
        .unwrap();
    fs::remove_dir_all(fixture.project()).unwrap();
    assert!(workspace.recover_missing_paths());
    assert!(workspace.state.root.is_none());
    assert_eq!(
        workspace.state.active_document().unwrap().content,
        "unsaved recovery text"
    );
    assert_eq!(workspace.state.unsaved_count(), 1);
}

#[test]
fn folder_picker_chooses_creation_parent_without_changing_the_open_project() {
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    app.open_path
        .set(fixture.project().to_string_lossy().into());
    app.project_name.set("picked-app".into());
    app.create();
    let picker = app.folder_picker(true);
    picker.complete(Ok(None));
    assert_eq!(
        app.project_parent.get(),
        fixture.project().to_string_lossy()
    );
    picker.complete(Ok(Some(vec![fixture.0.clone()])));
    assert_eq!(app.project_parent.get(), fixture.0.to_string_lossy());
    assert_eq!(app.open_path.get(), fixture.project().to_string_lossy());
    app.submit_create();
    app.flush();
    assert!(fixture.0.join("picked-app/Omen.toml").exists());
    assert!(!fixture.project().join("picked-app").exists());
}

#[test]
fn opening_a_deleted_recent_folder_removes_it_with_a_useful_error() {
    let fixture = Fixture::new();
    let missing = fixture.0.join("deleted-project");
    let mut workspace = fixture.workspace();
    workspace.state.recent.insert(0, missing.clone());
    let error = workspace.open(&missing).unwrap_err().to_string();
    assert!(error.contains("moved or deleted"));
    assert!(!workspace.state.recent.contains(&missing));
    assert_eq!(workspace.state.root, Some(fixture.project()));
}

#[test]
fn launcher_switches_between_onboarding_and_project_without_a_second_window() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{DesktopScene, Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn scenes(node: &ViewNode) -> Vec<DesktopScene> {
        let mut found = node.style.desktop_scenes.clone();
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                for child in children {
                    found.extend(scenes(child));
                }
            }
            _ => {}
        }
        found
    }
    fn contains(node: &ViewNode, label: &str) -> bool {
        match &node.kind {
            ViewKind::Button { label: text, .. } | ViewKind::Text(text) => text == label,
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().any(|child| contains(child, label))
            }
            _ => false,
        }
    }
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    let mut pool = LocalPool::new();
    let mut runtime = Runtime::new(
        omen_studio::views::StudioLauncher {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let home = runtime.render().unwrap();
    assert!(contains(&home, "Restoring workspace…"));
    assert!(!contains(&home, "Welcome to Omen Studio"));
    assert!(!contains(&home, "Sidebar"));
    assert!(!contains(&home, "Preview"));
    assert!(scenes(&home).is_empty());
    for job in runtime.take_tasks() {
        pool.spawner().spawn_local(job.run()).unwrap();
    }
    app.flush();
    pool.run_until_stalled();
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    pool.run_until_stalled();
    let editor = runtime.render().unwrap();
    assert!(contains(&editor, "Explorer"));
    assert!(!contains(&editor, "Welcome to Omen Studio"));
    assert!(
        scenes(&editor).is_empty(),
        "the project uses the primary window"
    );
    app.close_project();
    app.flush();
    pool.run_until_stalled();
    let home = runtime.render().unwrap();
    assert!(contains(&home, "Welcome to Omen Studio"));
    assert!(!contains(&home, "Explorer"));
    assert!(scenes(&home).is_empty());
    assert!(app.workspace.get().root.is_none());
    assert!(app.workspace.get().recent.contains(&fixture.project()));
}

#[test]
fn startup_creates_readable_settings_json_with_defaults() {
    let fixture = Fixture::new();
    let profile = fixture.0.join("settings");
    let (_service, mut updates) = WorkspaceService::start(profile.clone()).unwrap();
    let Event::Loaded(_, settings, _, _) = block_on(updates.next()).unwrap() else {
        panic!("Expected startup settings")
    };
    assert_eq!(settings, omen_studio::settings::StudioSettings::default());
    let source = fs::read_to_string(profile.join("settings.json")).unwrap();
    assert!(source.contains('\n'));
    let json: serde_json::Value = serde_json::from_str(&source).unwrap();
    assert_eq!(json["appearance"]["theme"], "xcode");
    assert_eq!(json["appearance"]["mode"], "dark");
}

#[test]
fn legacy_appearance_migrates_once_and_settings_then_take_precedence() {
    use omen_studio::{
        settings::{AppearanceMode, StudioSettings},
        theme::ThemePreset,
    };
    let fixture = Fixture::new();
    let profile = fixture.0.join("settings");
    fs::create_dir(&profile).unwrap();
    fs::write(
        profile.join("layout.json"),
        r#"{"preset":"midnight","dark":false,"sidebar_width":315,"sidebar_visible":false}"#,
    )
    .unwrap();
    let (service, mut updates) = WorkspaceService::start(profile.clone()).unwrap();
    let Event::Loaded(layout, settings, _, _) = block_on(updates.next()).unwrap() else {
        panic!("Expected migration")
    };
    assert_eq!(settings.appearance.theme, ThemePreset::Midnight);
    assert_eq!(settings.appearance.mode, AppearanceMode::Light);
    assert_eq!(layout.sidebar_width, 315.);
    assert!(!layout.sidebar_visible);
    let custom = StudioSettings::default();
    service.settings(custom.clone()).unwrap();
    service.flush().unwrap();
    drop(service);
    drop(updates);
    let (_service, mut updates) = WorkspaceService::start(profile).unwrap();
    let Event::Loaded(layout, settings, _, _) = block_on(updates.next()).unwrap() else {
        panic!("Expected settings load")
    };
    assert_eq!(settings, custom);
    assert_eq!(layout.sidebar_width, 315.);
    assert!(!layout.sidebar_visible);
}

#[test]
fn malformed_settings_are_reported_and_preserved_during_workspace_backups() {
    let fixture = Fixture::new();
    let profile = fixture.0.join("settings");
    fs::create_dir(&profile).unwrap();
    let invalid = r#"{"appearance":{"mode":"unknown"}}"#;
    fs::write(profile.join("settings.json"), invalid).unwrap();
    let (service, mut updates) = WorkspaceService::start(profile.clone()).unwrap();
    let Event::Loaded(_, settings, _, state) = block_on(updates.next()).unwrap() else {
        panic!("Expected startup")
    };
    assert_eq!(settings, omen_studio::settings::StudioSettings::default());
    assert!(state.message.contains("Settings recovery"));
    service.layout(LayoutPreferences::default()).unwrap();
    service.flush().unwrap();
    assert_eq!(
        fs::read_to_string(profile.join("settings.json")).unwrap(),
        invalid
    );
    service
        .settings(omen_studio::settings::StudioSettings::default())
        .unwrap();
    service.flush().unwrap();
    let Event::Changed(state) = block_on(updates.next()).unwrap() else {
        panic!("Expected recovery acknowledgement")
    };
    assert_eq!(state.message, "Ready");
    let repaired: omen_studio::settings::StudioSettings =
        services::read_json(&profile.join("settings.json")).unwrap();
    assert_eq!(repaired, omen_studio::settings::StudioSettings::default());
}

#[test]
fn partial_settings_use_defaults_and_load_without_legacy_appearance() {
    let fixture = Fixture::new();
    let profile = fixture.0.join("settings");
    fs::create_dir(&profile).unwrap();
    fs::write(
        profile.join("settings.json"),
        r#"{"appearance":{"theme":"graphite"}}"#,
    )
    .unwrap();
    let (_service, mut updates) = WorkspaceService::start(profile).unwrap();
    let Event::Loaded(_, settings, _, _) = block_on(updates.next()).unwrap() else {
        panic!("Expected settings")
    };
    assert_eq!(
        settings.appearance.theme,
        omen_studio::theme::ThemePreset::Graphite
    );
    assert_eq!(
        settings.appearance.mode,
        omen_studio::settings::AppearanceMode::Dark
    );
}

#[test]
fn project_form_edits_identity_without_exposing_a_toml_editor() {
    use omen::{Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(label)
            || matches!(&node.kind, ViewKind::Button { label: text, .. } | ViewKind::TextField { label: text, .. } if text == label)
        {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Presentation { children, .. } => {
                children.iter().rev().find_map(|child| find(child, label))
            }
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children) => children.iter().find_map(|child| find(child, label)),
            _ => None,
        }
    }
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("settings"));
    app.open_path.set(fixture.0.to_string_lossy().into());
    app.create();
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let form = runtime.render().unwrap();
    assert!(find(&form, "Omen.toml").is_none());
    find(&form, "Project name")
        .unwrap()
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("polished-app".into());
    let defaults: omen_cli::project::manifest::Manifest =
        toml::from_str(&app.manifest_source.get()).unwrap();
    assert_eq!(defaults.app.name, "polished-app");
    assert_eq!(defaults.app.id, "com.example.polishedapp");
    let form = runtime.render().unwrap();
    find(&form, "Application ID")
        .unwrap()
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("invalid".into());
    app.submit_create();
    assert!(!fixture.0.join("polished-app").exists());
    assert!(!app.creation_error.get().is_empty());
    find(&form, "Application ID")
        .unwrap()
        .style
        .text_value
        .as_ref()
        .unwrap()
        .set("org.example.polished".into());
    app.project_name_input().set("final-app".into());
    let form = runtime.render().unwrap();
    let ViewKind::Button { action, .. } = &find(&form, "Create project").unwrap().kind else {
        panic!("Expected primary create action")
    };
    action.invoke();
    app.flush();
    let source = fs::read_to_string(fixture.0.join("final-app/Omen.toml")).unwrap();
    let saved: omen_cli::project::manifest::Manifest = toml::from_str(&source).unwrap();
    assert_eq!(saved.app.name, "final-app");
    assert_eq!(
        saved.app.id, "org.example.polished",
        "custom identifiers survive project renaming"
    );
    assert_eq!(saved.window.title, "Final App");
}

#[test]
fn explorer_creation_creates_folders_opens_files_and_refuses_overwrites() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    workspace
        .apply(Command::NewEntry {
            parent: fixture.project(),
            name: "assets".into(),
            directory: true,
        })
        .unwrap();
    let parent = fixture.project().join("assets");
    workspace
        .apply(Command::NewEntry {
            parent: parent.clone(),
            name: "hello.rs".into(),
            directory: false,
        })
        .unwrap();
    let file = parent.join("hello.rs");
    assert!(workspace.state.expanded.contains(&parent));
    assert!(workspace.state.rows.iter().any(|row| row.path == file));
    assert_eq!(workspace.state.active_file, Some(file.clone()));
    workspace
        .apply(Command::Edit {
            path: file.clone(),
            content: "// Keep this work".into(),
        })
        .unwrap();
    workspace.apply(Command::Save(file.clone())).unwrap();
    let error = workspace
        .apply(Command::NewEntry {
            parent,
            name: "hello.rs".into(),
            directory: false,
        })
        .unwrap_err();
    assert!(error.to_string().contains("already exists"));
    assert_eq!(fs::read_to_string(file).unwrap(), "// Keep this work");
    assert!(
        workspace
            .apply(Command::NewEntry {
                parent: fixture.project(),
                name: "assets".into(),
                directory: true
            })
            .is_err()
    );
}

#[test]
fn explorer_creation_rejects_path_traversal_and_outside_parents() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "src/escape",
        "/absolute",
        "bad\\name",
        "CON",
        "bad:name",
        "trailing.",
    ] {
        assert!(
            workspace
                .apply(Command::NewEntry {
                    parent: fixture.project(),
                    name: name.into(),
                    directory: false
                })
                .is_err(),
            "{name}"
        );
    }
    assert!(
        workspace
            .apply(Command::NewEntry {
                parent: fixture.0.clone(),
                name: "escape".into(),
                directory: true
            })
            .is_err()
    );
    assert!(!fixture.0.join("escape").exists());
    workspace
        .apply(Command::NewEntry {
            parent: fixture.project(),
            name: "résumé.txt".into(),
            directory: false,
        })
        .unwrap();
    assert!(fixture.project().join("résumé.txt").exists());
}

#[test]
fn explorer_icon_actions_create_entries_refresh_and_collapse_the_tree() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(label)
            || matches!(&node.kind, ViewKind::Button { label: text, .. } if text == label)
        {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| find(child, label))
            }
            _ => None,
        }
    }
    fn click(node: &ViewNode, label: &str) {
        let node = find(node, label).unwrap();
        if let Some(action) = &node.style.on_tap {
            action.invoke();
        } else if let ViewKind::Button { action, .. } = &node.kind {
            action.invoke();
        } else {
            panic!("Expected an explorer action")
        }
    }
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let root = runtime.render().unwrap();
    assert!(find(&root, "Refresh").is_none());
    assert!(find(&root, "Create New").is_some());
    assert!(find(&root, "Refresh explorer").is_some());
    click(&root, "New folder");
    assert!(app.new_entry_visible.get());
    app.entry_name().set("assets".into());
    click(&runtime.render().unwrap(), "Create");
    app.flush();
    pool.run_until_stalled();
    assert!(fixture.project().join("assets").is_dir());
    assert!(!app.new_entry_visible.get());
    click(&runtime.render().unwrap(), "Create New");
    assert!(app.palette_visible.get());
    click(&runtime.render().unwrap(), "New Rust File");
    block_on(app.clone().inspect_entry());
    app.entry_registration().set(false);
    app.entry_name().set("notes.rs".into());
    click(&runtime.render().unwrap(), "Create");
    app.flush();
    pool.run_until_stalled();
    assert_eq!(
        app.workspace.get().active_file,
        Some(fixture.project().join("notes.rs"))
    );
    fs::write(fixture.project().join("external.rs"), "// external").unwrap();
    click(&runtime.render().unwrap(), "Refresh explorer");
    app.flush();
    pool.run_until_stalled();
    assert!(
        app.workspace
            .get()
            .rows
            .iter()
            .any(|row| row.path.ends_with("external.rs"))
    );
    click(&runtime.render().unwrap(), "Collapse project");
    let collapsed = runtime.render().unwrap();
    assert!(
        find(
            &collapsed,
            &fixture.project().join("notes.rs").display().to_string()
        )
        .is_none()
    );
    click(&collapsed, "Expand project");
    assert!(
        find(
            &runtime.render().unwrap(),
            &fixture.project().join("notes.rs").display().to_string()
        )
        .is_some()
    );
}

#[test]
fn explorer_uses_left_aligned_svg_rows_and_selects_folders_and_files() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Alignment, Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn find<'a>(node: &'a ViewNode, path: &Path) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(path.to_string_lossy().as_ref()) {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| find(child, path))
            }
            _ => None,
        }
    }
    fn has_icon(node: &ViewNode, name: &str) -> bool {
        match &node.kind {
            ViewKind::Icon(source) => source == name,
            ViewKind::Group(children) | ViewKind::Layout(children) => {
                children.iter().any(|child| has_icon(child, name))
            }
            _ => false,
        }
    }
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let root = runtime.render().unwrap();
    let folder = fixture.project().join("src");
    let row = find(&root, &folder).unwrap();
    assert!(
        matches!(row.kind, ViewKind::Layout(_)),
        "rows use an actual layout rather than centered button labels"
    );
    assert_eq!(row.style.alignment, Some(Alignment::Leading));
    assert!(has_icon(row, "folder"));
    row.style.on_tap.as_ref().unwrap().invoke();
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.explorer_selection.get(), Some(folder.clone()));
    assert!(app.workspace.get().active_file.is_none());
    let root = runtime.render().unwrap();
    let row = find(&root, &folder).unwrap();
    assert!(has_icon(row, "folder-open"));
    assert_eq!(row.style.background, Some(app.colors().selection.into()));
    let file = find(&root, &fixture.file()).unwrap();
    assert!(has_icon(file, "file-rust"));
    file.style.on_tap.as_ref().unwrap().invoke();
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file, Some(fixture.file()));
    assert_eq!(app.explorer_selection.get(), Some(fixture.file()));
    let root = runtime.render().unwrap();
    assert_eq!(
        find(&root, &fixture.file()).unwrap().style.background,
        Some(app.colors().selection.into())
    );
    assert_eq!(
        find(&root, &folder).unwrap().style.background,
        Some(app.colors().surface_container.into())
    );
}

#[test]
fn native_titlebar_menus_replace_toolbar_controls_and_route_project_actions() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, View, ViewKind, ViewNode};
    fn find<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if node.style.accessibility.label.as_deref() == Some(label)
            || matches!(&node.kind, ViewKind::Button { label: text, .. } | ViewKind::TextField { label: text, .. } if text == label)
        {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| find(child, label))
            }
            _ => None,
        }
    }
    fn click(node: &ViewNode, label: &str) {
        let ViewKind::Button { action, .. } = &find(node, label).unwrap().kind else {
            panic!("Expected menu action")
        };
        action.invoke();
    }
    let fixture = Fixture::new();
    let mut manifest = omen_cli::project::template::default_manifest("studio-run-test", None);
    use omen_cli::project::manifest::TargetPlatform;
    manifest.targets.platforms = vec![TargetPlatform::Desktop];
    fs::write(
        fixture.project().join("Omen.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.project()));
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let root = runtime.render().unwrap();
    assert!(find(&root, "Project directory").is_none());
    assert!(find(&root, "New project name").is_none());
    assert!(find(&root, "Browse…").is_none());
    assert!(
        find(&root, "Save").is_none(),
        "file actions only appear inside their menu"
    );
    click(&root, "File");
    let menu = runtime.render().unwrap();
    for label in ["New project", "Open project", "Save", "Save all"] {
        assert!(find(&menu, label).is_some(), "{label}");
    }
    click(&menu, "Window");
    let menu = runtime.render().unwrap();
    assert!(
        find(&menu, "New project").is_none(),
        "only one menu can be open"
    );
    click(&menu, "Toggle sidebar");
    assert!(!app.layout.get().sidebar_visible);
    click(&runtime.render().unwrap(), "Window");
    click(&runtime.render().unwrap(), "Toggle inspector");
    assert!(!app.layout.get().preview_visible);
    click(&runtime.render().unwrap(), "File");
    find(&runtime.render().unwrap(), "Open project")
        .unwrap()
        .style
        .file_picker
        .as_ref()
        .unwrap()
        .complete(Ok(None));
    assert!(find(&runtime.render().unwrap(), "New project").is_none());
    click(&runtime.render().unwrap(), "File");
    click(&runtime.render().unwrap(), "New project");
    assert!(app.creation_visible.get());
    app.cancel_create();
    app.content(fixture.file())
        .set("// saved from File menu\n".into());
    app.flush();
    pool.run_until_stalled();
    click(&runtime.render().unwrap(), "File");
    click(&runtime.render().unwrap(), "Save all");
    app.flush();
    pool.run_until_stalled();
    assert_eq!(
        fs::read_to_string(fixture.file()).unwrap(),
        "// saved from File menu\n"
    );
    assert!(find(&runtime.render().unwrap(), "Save all").is_none());
    click(&runtime.render().unwrap(), "Choose a run device");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        pool.run_until_stalled();
        if find(&runtime.render().unwrap(), "Run on This computer").is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Desktop run target did not appear"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    click(&runtime.render().unwrap(), "File");
    assert!(find(&runtime.render().unwrap(), "Run on This computer").is_none());

    click(&runtime.render().unwrap(), "Show Debug Console");
    assert!(find(&runtime.render().unwrap(), "Close Debug Console panel").is_some());
    click(&runtime.render().unwrap(), "Close Debug Console panel");
    assert!(find(&runtime.render().unwrap(), "Close Debug Console panel").is_none());
}

#[test]
fn persistent_editor_find_replace_and_restart_preserve_text() {
    let fixture = Fixture::new();
    let profile = fixture.0.join("editor-profile");
    let app = StudioEnvironment::at(profile.clone());
    app.send(Command::OpenProject(fixture.project()));
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    // Drive the normal foreground delivery once, retaining its cancellation-safe stream.
    let mut pool = futures::executor::LocalPool::new();
    use futures::task::LocalSpawnExt;
    pool.spawner().spawn_local(app.receive()).unwrap();
    pool.run_until_stalled();
    let document = app.editor_document(fixture.file());
    app.find_text.set("println".into());
    futures::executor::block_on(app.find_next());
    let state = document.state.get();
    assert_eq!(state.text.slice(state.selection), "println");
    app.replacement_text.set("eprintln".into());
    futures::executor::block_on(app.replace_match(false));
    app.flush();
    pool.run_until_stalled();
    assert!(
        app.workspace
            .get()
            .active_document()
            .unwrap()
            .content
            .contains("eprintln!")
    );
    assert_eq!(app.workspace.get().unsaved_count(), 1);
    app.send(Command::Save(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().unsaved_count(), 0);
    let restored: WorkspaceState = services::read_json(&profile.join("workspace.json")).unwrap();
    assert_eq!(
        restored.unsaved_count(),
        0,
        "clean snapshots remain clean after JSON restoration"
    );
}

#[test]
fn language_workspace_edits_are_staged_version_checked_and_confined_to_project() {
    use omen_studio::editor::lsp::protocol;
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let file = fixture.file();
    let expected = std::collections::HashMap::from([(
        file.clone(),
        workspace.state.active_document().unwrap().content.clone(),
    )]);
    let edit = serde_json::json!({"changes":{protocol::uri(&file):[{"range":{"start":{"line":0,"character":3},"end":{"line":0,"character":7}},"newText":"renamed"}]}});
    workspace
        .apply(Command::ApplyLanguageEdit {
            edit: edit.clone(),
            expected: expected.clone(),
        })
        .unwrap();
    assert!(
        workspace
            .state
            .active_document()
            .unwrap()
            .content
            .contains("fn renamed")
    );
    assert!(
        fs::read_to_string(&file).unwrap().contains("fn main"),
        "language edits stay unsaved for review"
    );
    let before = workspace.state.active_document().unwrap().content.clone();
    assert!(
        workspace
            .apply(Command::ApplyLanguageEdit { edit, expected })
            .is_err(),
        "stale request snapshots must be rejected"
    );
    assert_eq!(workspace.state.active_document().unwrap().content, before);
    let outside = fixture.0.join("outside.rs");
    fs::write(&outside, "keep").unwrap();
    let edit = serde_json::json!({"changes":{protocol::uri(&outside):[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":4}},"newText":"lost"}]}});
    assert!(
        workspace
            .apply(Command::ApplyLanguageEdit {
                edit,
                expected: Default::default()
            })
            .is_err()
    );
    assert_eq!(fs::read_to_string(outside).unwrap(), "keep");
}

#[test]
fn explorer_rename_preserves_unsaved_buffers_and_remaps_folder_tabs() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "// unsaved\n".into(),
        })
        .unwrap();
    workspace
        .apply(Command::RenameEntry {
            path: fixture.project().join("src"),
            name: "sources".into(),
        })
        .unwrap();
    let renamed = fixture.project().join("sources/main.rs");
    assert_eq!(workspace.state.active_file, Some(renamed.clone()));
    assert_eq!(workspace.state.documents[0].path, renamed);
    assert_eq!(workspace.state.documents[0].content, "// unsaved\n");
    assert!(workspace.state.documents[0].dirty());
    assert!(!fixture.project().join("src").exists());
    fs::write(fixture.project().join("sources/taken.rs"), "keep").unwrap();
    assert!(
        workspace
            .apply(Command::RenameEntry {
                path: renamed.clone(),
                name: "taken.rs".into()
            })
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(fixture.project().join("sources/taken.rs")).unwrap(),
        "keep"
    );
    assert!(
        workspace
            .apply(Command::RenameEntry {
                path: renamed,
                name: "../escape".into()
            })
            .is_err()
    );
    assert!(
        workspace
            .apply(Command::RenameEntry {
                path: fixture.project(),
                name: "other".into()
            })
            .is_err()
    );
}

#[test]
fn explorer_copy_and_cut_paste_preserve_files_and_refuse_unsafe_moves() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let src = fixture.project().join("src");
    workspace
        .apply(Command::PasteEntry {
            source: fixture.file(),
            parent: src.clone(),
            cut: false,
        })
        .unwrap();
    assert_eq!(
        fs::read(src.join("main copy.rs")).unwrap(),
        fs::read(fixture.file()).unwrap()
    );
    fs::create_dir(fixture.project().join("dest")).unwrap();
    let dest = fixture.project().join("dest");
    workspace
        .apply(Command::PasteEntry {
            source: fixture.file(),
            parent: dest.clone(),
            cut: true,
        })
        .unwrap();
    assert!(!fixture.file().exists());
    assert_eq!(workspace.state.active_file, Some(dest.join("main.rs")));
    assert_eq!(workspace.state.documents[0].path, dest.join("main.rs"));
    workspace
        .apply(Command::Edit {
            path: dest.join("main.rs"),
            content: "unsaved".into(),
        })
        .unwrap();
    assert!(
        workspace
            .apply(Command::PasteEntry {
                source: dest.clone(),
                parent: src.clone(),
                cut: true
            })
            .is_err()
    );
    assert!(
        workspace
            .apply(Command::PasteEntry {
                source: dest.clone(),
                parent: dest.clone(),
                cut: false
            })
            .is_err()
    );
    assert!(workspace.apply(Command::TrashEntry(dest)).is_err());
    assert!(
        workspace
            .apply(Command::TrashEntry(fixture.project()))
            .is_err()
    );
    assert!(
        workspace
            .apply(Command::PasteEntry {
                source: fixture.0.join("project"),
                parent: src,
                cut: false
            })
            .is_err()
    );
}

#[test]
fn explorer_context_actions_route_to_worker_and_keep_root_actions_safe() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn button<'a>(node: &'a ViewNode, label: &str) -> Option<&'a ViewNode> {
        if matches!(&node.kind,ViewKind::Button {label:text,..} if text==label) {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|child| button(child, label))
            }
            _ => None,
        }
    }
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.project()));
    app.flush();
    pool.run_until_stalled();
    app.send(Command::OpenFile(fixture.file()));
    app.flush();
    pool.run_until_stalled();
    let row = omen_studio::state::TreeRow {
        path: fixture.file(),
        depth: 1,
        directory: false,
        expanded: false,
    };
    app.show_context(row.clone());
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let rendered = runtime.render().unwrap();
    assert!(button(&rendered, "Rename…").is_some());
    assert!(button(&rendered, "Move to Trash…").is_some());
    assert!(matches!(
        button(&rendered, "Copy path").unwrap().style.desktop_action,
        Some(omen::DesktopAction::CopyText(_))
    ));
    app.rename_context();
    block_on(app.clone().inspect_entry());
    assert_eq!(app.new_entry.get().source, Some(fixture.file()));
    app.entry_name().set("renamed.rs".into());
    app.submit_new_entry();
    app.flush();
    pool.run_until_stalled();
    let renamed = fixture.project().join("src/renamed.rs");
    assert!(renamed.exists());
    assert!(!app.new_entry_visible.get());
    assert_eq!(app.explorer_selection.get(), Some(renamed.clone()));
    app.show_context(omen_studio::state::TreeRow {
        path: renamed,
        ..row
    });
    app.copy_context_entry(false);
    app.show_context(omen_studio::state::TreeRow {
        path: fixture.project(),
        depth: 0,
        directory: true,
        expanded: true,
    });
    let rendered = runtime.render().unwrap();
    assert!(button(&rendered, "Rename…").is_none());
    assert!(button(&rendered, "Move to Trash…").is_none());
    assert!(button(&rendered, "Paste").is_some());
    app.paste_context_entry();
    app.flush();
    pool.run_until_stalled();
    assert!(fixture.project().join("renamed.rs").exists());
    app.new_context_entry(true);
    assert_eq!(app.new_entry.get().parent, Some(fixture.project()));
    assert!(app.new_entry.get().directory);
}

#[test]
fn explorer_recursive_copy_preserves_contents_and_rejects_links_before_writing() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    let src = fixture.project().join("src");
    fs::create_dir(src.join("nested")).unwrap();
    fs::write(src.join("nested/data.bin"), [0, 1, 255]).unwrap();
    workspace
        .apply(Command::PasteEntry {
            source: src.clone(),
            parent: fixture.project(),
            cut: false,
        })
        .unwrap();
    let copied = fixture.project().join("src copy");
    assert_eq!(
        fs::read(copied.join("nested/data.bin")).unwrap(),
        vec![0, 1, 255]
    );
    assert_eq!(
        fs::read(copied.join("main.rs")).unwrap(),
        fs::read(fixture.file()).unwrap()
    );
    assert_eq!(workspace.state.active_file, Some(fixture.file()));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(fixture.file(), src.join("linked.rs")).unwrap();
        assert!(
            workspace
                .apply(Command::PasteEntry {
                    source: src,
                    parent: fixture.project(),
                    cut: false
                })
                .is_err()
        );
        assert!(!fixture.project().join("src copy 2").exists());
    }
}

#[test]
fn startup_restores_last_open_project_and_explicit_close_persists_onboarding() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn contains(node: &ViewNode, label: &str) -> bool {
        match &node.kind {
            ViewKind::Text(text) => text == label,
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::Card(children)
            | ViewKind::List(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().any(|node| contains(node, label))
            }
            _ => false,
        }
    }
    let fixture = Fixture::new();
    let profile = fixture.0.join("profile");
    let (service, mut events) = WorkspaceService::start(profile.clone()).unwrap();
    block_on(events.next()).unwrap();
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    service.flush().unwrap();
    drop(service);
    drop(events);
    let app = StudioEnvironment::at(profile.clone());
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.flush();
    pool.run_until_stalled();
    assert!(app.editor_visible.get());
    assert_eq!(app.workspace.get().root, Some(fixture.project()));
    let mut runtime = Runtime::new(
        omen_studio::views::StudioLauncher {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let root = runtime.render().unwrap();
    assert!(contains(&root, "Explorer"));
    assert!(!contains(&root, "Welcome to Omen Studio"));
    app.close_project();
    app.flush();
    pool.run_until_stalled();
    assert!(!app.editor_visible.get());
    assert!(contains(
        &runtime.render().unwrap(),
        "Welcome to Omen Studio"
    ));
    app.flush();
    let (restored, mut events) = WorkspaceService::start(profile).unwrap();
    let Event::Loaded(_, _, _, state) = block_on(events.next()).unwrap() else {
        panic!("Expected restored workspace")
    };
    assert!(state.root.is_none());
    assert!(state.documents.is_empty());
    assert!(state.recent.contains(&fixture.project()));
    drop(restored);
}
#[test]
fn close_project_does_not_discard_unsaved_documents() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    workspace
        .apply(Command::Edit {
            path: fixture.file(),
            content: "// unsaved\n".into(),
        })
        .unwrap();
    assert!(workspace.apply(Command::CloseProject).is_err());
    assert_eq!(workspace.state.root, Some(fixture.project()));
    assert_eq!(workspace.state.documents[0].content, "// unsaved\n");
}

#[test]
fn launcher_keeps_one_event_task_when_switching_to_the_workspace() {
    use omen::{Runtime, Value, View, ViewExt};
    use omen_studio::views::StudioLauncher;
    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("single-owner"));
    let mut runtime = Runtime::new(
        StudioLauncher {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    runtime.render().unwrap();
    assert_eq!(runtime.take_tasks().len(), 1);
    app.ready.set(true);
    app.editor_visible.set(true);
    runtime.render().unwrap();
    assert!(
        runtime.take_tasks().is_empty(),
        "opening the shell must not start a second workspace receiver"
    );
}

#[test]
fn autosave_waits_for_the_latest_edit_and_off_cancels_the_deadline() {
    use omen_studio::settings::{AutoSaveMode, StudioSettings};
    use std::time::{Duration, Instant};

    let fixture = Fixture::new();
    let path = fixture.file();
    let original = fs::read_to_string(&path).unwrap();
    let (service, _updates) = WorkspaceService::start(fixture.0.join("autosave")).unwrap();
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    service.send(2, Command::OpenFile(path.clone())).unwrap();
    let mut settings = StudioSettings::default();
    settings.editor.auto_save = AutoSaveMode::AfterDelay;
    settings.editor.auto_save_delay_ms = 500;
    service.settings(settings.clone()).unwrap();
    let edit = |sequence, content: &str| {
        service
            .send(
                sequence,
                Command::Edit {
                    path: path.clone(),
                    content: content.into(),
                },
            )
            .unwrap();
        service.flush().unwrap();
    };

    edit(3, "first");
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    std::thread::sleep(Duration::from_millis(80));
    edit(4, "latest");
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let deadline = Instant::now() + Duration::from_secs(3);
    while fs::read_to_string(&path).unwrap() != "latest" {
        assert!(
            Instant::now() < deadline,
            "idle save did not write the latest edit"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    edit(5, "manual");
    settings.editor.auto_save = AutoSaveMode::Off;
    service.settings(settings).unwrap();
    service.editor_blur(path.clone()).unwrap();
    service.window_blur().unwrap();
    service.flush().unwrap();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(fs::read_to_string(path).unwrap(), "latest");
}

#[test]
fn autosave_focus_modes_preserve_conflicts_and_save_the_expected_files() {
    use omen_studio::settings::{AutoSaveMode, StudioSettings};

    let fixture = Fixture::new();
    let first = fixture.file();
    let second = fixture.project().join("src/second.rs");
    fs::write(&second, "old").unwrap();
    let (service, _updates) = WorkspaceService::start(fixture.0.join("autosave")).unwrap();
    service
        .send(1, Command::OpenProject(fixture.project()))
        .unwrap();
    service.send(2, Command::OpenFile(first.clone())).unwrap();
    service.send(3, Command::OpenFile(second.clone())).unwrap();
    let mut settings = StudioSettings::default();
    settings.editor.auto_save = AutoSaveMode::OnFocusChange;
    service.settings(settings.clone()).unwrap();
    let edit = |sequence, path: &Path, content: &str| {
        service
            .send(
                sequence,
                Command::Edit {
                    path: path.to_owned(),
                    content: content.into(),
                },
            )
            .unwrap();
    };
    edit(4, &first, "first draft");
    edit(5, &second, "second draft");
    service.editor_blur(first.clone()).unwrap();
    service.flush().unwrap();
    assert_eq!(fs::read_to_string(&first).unwrap(), "first draft");
    assert_eq!(fs::read_to_string(&second).unwrap(), "old");

    service.send(6, Command::Select(first.clone())).unwrap();
    service.flush().unwrap();
    assert_eq!(fs::read_to_string(&second).unwrap(), "second draft");

    settings.editor.auto_save = AutoSaveMode::OnWindowChange;
    service.settings(settings).unwrap();
    edit(7, &first, "conflicting draft");
    edit(8, &second, "saved on window blur");
    service.editor_blur(second.clone()).unwrap();
    service.flush().unwrap();
    assert_eq!(fs::read_to_string(&second).unwrap(), "second draft");
    fs::write(&first, "external edit").unwrap();
    service.window_blur().unwrap();
    service.flush().unwrap();
    assert_eq!(fs::read_to_string(first).unwrap(), "external edit");
    assert_eq!(fs::read_to_string(second).unwrap(), "saved on window blur");
}

#[test]
fn studio_creation_adds_account_headers_to_starters_and_new_files() {
    use omen_cli::project::template::default_manifest;
    use omen_studio::{account::Account, workspace::file_kind::StudioFileKind};

    let fixture = Fixture::new();
    let profile = fixture.0.join("account");
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let app = fixture.0.join("example");
    let (service, _updates) = WorkspaceService::start(profile.clone()).unwrap();
    let account = Account {
        name: "Ada Lovelace".into(),
        email: "ada@example.test".into(),
    };
    service.account(account).unwrap();
    let mut manifest = default_manifest("example", Some(sdk.clone()));
    manifest.app.name = "Example App".into();
    service
        .send(
            1,
            Command::CreateConfigured {
                name: "example".into(),
                parent: fixture.0.clone(),
                sdk,
                source: toml::to_string(&manifest).unwrap(),
            },
        )
        .unwrap();

    service
        .send(
            2,
            Command::CreateTyped {
                parent: app.join("src"),
                kind: StudioFileKind::View,
                name: "dashboard".into(),
                registration: None,
            },
        )
        .unwrap();
    for (sequence, name) in [(3, "notes.md"), (4, "data.json")] {
        service
            .send(
                sequence,
                Command::NewEntry {
                    parent: app.clone(),
                    name: name.into(),
                    directory: false,
                },
            )
            .unwrap();
    }
    service.flush().unwrap();
    for (file, prefix) in [
        ("src/main.rs", "// "),
        ("src/views/mod.rs", "// "),
        ("src/views/home_view.rs", "// "),
        ("src/dashboard_view.rs", "// "),
        ("Cargo.toml", "# "),
        ("Omen.toml", "# "),
        (".gitignore", "# "),
        ("notes.md", "  "),
    ] {
        let text = fs::read_to_string(app.join(file)).unwrap();
        assert!(text.contains(&format!(
            "{prefix}File name: {}",
            Path::new(file).file_name().unwrap().to_string_lossy()
        )));
        assert!(text.contains(&format!("{prefix}App name: Example App")));
        assert!(text.contains(&format!("{prefix}Created by: Ada Lovelace")));
        let created_at = text
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{prefix}Created at: ")))
            .unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(created_at).is_ok());
        assert!(!text.contains("ada@example.test"));
    }
    assert!(
        fs::read_to_string(app.join("data.json"))
            .unwrap()
            .is_empty()
    );
    assert!(
        fs::read_to_string(app.join("notes.md"))
            .unwrap()
            .starts_with("<!--\n")
    );
    let toml = fs::read_to_string(app.join("Cargo.toml")).unwrap();
    assert_eq!(
        toml::from_str::<toml::Value>(&toml).unwrap()["package"]["name"].as_str(),
        Some("example")
    );
}
