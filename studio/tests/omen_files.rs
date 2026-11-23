use omen_studio::{
    state::WorkspaceState,
    workspace::{
        Command, Workspace,
        file_kind::{FileNames, StudioFileKind as Kind},
        modules::ModuleTarget,
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
        let root = std::env::temp_dir().join(format!(
            "omen-aware-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"),"[package]\nname=\"omen_generated_files\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n").unwrap();
        fs::write(root.join("src/lib.rs"), "//! Crate documentation.\n").unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn workspace(&self) -> Workspace {
        let mut workspace = Workspace::new(WorkspaceState::default());
        workspace.open(&self.0).unwrap();
        workspace
    }
    fn src(&self) -> PathBuf {
        self.0.join("src")
    }
    fn target(&self, workspace: &Workspace, parent: &Path) -> ModuleTarget {
        let targets = workspace.module_targets(parent).unwrap();
        assert_eq!(targets.len(), 1);
        targets[0].clone()
    }
    fn create(&self, workspace: &mut Workspace, parent: &Path, kind: Kind, name: &str) {
        let registration = Some(self.target(workspace, parent));
        workspace
            .apply(Command::CreateTyped {
                parent: parent.into(),
                kind,
                name: name.into(),
                registration,
            })
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn exact_suffix_classification_and_icons_are_distinct() {
    for (name, kind) in [
        ("dashboard_view.rs", Kind::View),
        ("profile_state.rs", Kind::State),
        ("app_env.rs", Kind::Environment),
        ("en_locale.rs", Kind::Localization),
        ("mod.rs", Kind::Module),
        ("view.rs", Kind::Rust),
        ("foo_mod.rs", Kind::Rust),
        ("main.rs", Kind::Rust),
        ("foo_view.rs.bak", Kind::Other),
        ("foo_view.RS", Kind::Other),
    ] {
        assert_eq!(Kind::classify(Path::new(name), false), kind);
        assert_eq!(Kind::classify(Path::new(name), true), Kind::Folder);
    }
    let icons: std::collections::BTreeSet<_> = [
        Kind::View,
        Kind::State,
        Kind::Environment,
        Kind::Localization,
        Kind::Module,
        Kind::Rust,
    ]
    .map(Kind::icon)
    .into_iter()
    .collect();
    assert_eq!(icons.len(), 6);
    for kind in [
        Kind::View,
        Kind::State,
        Kind::Environment,
        Kind::Localization,
        Kind::Module,
    ] {
        let svg = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/omen-gpui/assets/icons")
            .join(format!("{}.svg", kind.icon()));
        assert!(fs::read_to_string(svg).unwrap().contains("<svg"));
        assert_ne!(kind.tint(true), kind.tint(false));
    }
}
#[test]
fn names_and_templates_are_deterministic_and_reject_invalid_paths() {
    for (input, stem, ty) in [
        ("Profile", "profile_view.rs", "Profile"),
        ("HTTP Server", "http_server_view.rs", "HttpServer"),
        ("profile_view.rs", "profile_view.rs", "Profile"),
        ("my-profile", "my_profile_view.rs", "MyProfile"),
    ] {
        let names = FileNames::new(input, Kind::View).unwrap();
        assert_eq!(names.file_name, stem);
        assert_eq!(names.type_name, ty);
        assert!(names.render(Kind::View).contains(&format!("pub {ty}")));
    }
    for input in [
        "",
        "../escape",
        "/tmp/x",
        "123name",
        "foo.bar",
        "bad\\name",
        "$value",
    ] {
        assert!(FileNames::new(input, Kind::View).is_err(), "{input}");
    }
    assert!(FileNames::new("mod", Kind::Rust).is_err());
    assert_eq!(
        FileNames::new("Profile", Kind::State).unwrap().type_name,
        "ProfileState"
    );
}
#[test]
fn creates_view_and_module_registers_and_opens_without_overwrites() {
    let fixture = Fixture::new();
    let mut workspace = fixture.workspace();
    fixture.create(&mut workspace, &fixture.src(), Kind::View, "Dashboard");
    assert!(
        fs::read_to_string(fixture.src().join("dashboard_view.rs"))
            .unwrap()
            .contains("pub Dashboard")
    );
    assert!(
        fs::read_to_string(fixture.src().join("lib.rs"))
            .unwrap()
            .contains("pub mod dashboard_view;")
    );
    assert_eq!(
        workspace.state.active_file,
        Some(fixture.src().join("dashboard_view.rs"))
    );
    let target = fixture.target(&workspace, &fixture.src());
    assert!(
        workspace
            .apply(Command::CreateTyped {
                parent: fixture.src(),
                kind: Kind::View,
                name: "Dashboard".into(),
                registration: Some(target)
            })
            .is_err()
    );
    fixture.create(&mut workspace, &fixture.src(), Kind::Module, "Settings");
    assert!(fixture.src().join("settings/mod.rs").is_file());
    fixture.create(
        &mut workspace,
        &fixture.src().join("settings"),
        Kind::State,
        "App",
    );
    assert!(
        fs::read_to_string(fixture.src().join("settings/mod.rs"))
            .unwrap()
            .contains("pub mod app_state;")
    );
}
#[test]
fn modern_file_modules_and_inline_modules_resolve_to_correct_directories() {
    let fixture = Fixture::new();
    fs::write(
        fixture.src().join("lib.rs"),
        "mod modern;\nmod inline { pub fn marker() {} }\n",
    )
    .unwrap();
    fs::write(fixture.src().join("modern.rs"), "//! modern\n").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(
        &mut workspace,
        &fixture.src().join("modern.rs"),
        Kind::Module,
        "Nested",
    );
    assert!(fixture.src().join("modern/nested/mod.rs").exists());
    assert!(
        fs::read_to_string(fixture.src().join("modern.rs"))
            .unwrap()
            .contains("pub mod nested;")
    );
    let targets = workspace
        .module_targets(&fixture.src().join("lib.rs"))
        .unwrap();
    assert!(targets.len() > 1);
    let target = targets
        .into_iter()
        .find(|target| target.inline == ["inline"])
        .unwrap();
    workspace
        .apply(Command::CreateTyped {
            parent: fixture.src().join("lib.rs"),
            kind: Kind::View,
            name: "Inner".into(),
            registration: Some(target),
        })
        .unwrap();
    assert!(fixture.src().join("inline/inner_view.rs").exists());
    let source = fs::read_to_string(fixture.src().join("lib.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();
    let items =
        omen_studio::workspace::modules::scope_items(&syntax.items, &["inline".into()]).unwrap();
    assert!(
        items
            .iter()
            .any(|item| matches!(item,syn::Item::Mod(module) if module.ident=="inner_view"))
    );
}
#[test]
fn refuses_undeclared_folders_ambiguous_layouts_and_duplicate_declarations() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.src().join("orphan")).unwrap();
    let mut workspace = fixture.workspace();
    assert!(
        workspace
            .module_targets(&fixture.src().join("orphan"))
            .is_err()
    );
    fs::write(fixture.src().join("lib.rs"), "pub mod missing;\n").unwrap();
    let target = fixture.target(&workspace, &fixture.src());
    assert!(
        workspace
            .apply(Command::CreateTyped {
                parent: fixture.src(),
                kind: Kind::Rust,
                name: "Missing".into(),
                registration: Some(target)
            })
            .is_err()
    );
    assert!(!fixture.src().join("missing.rs").exists());
    fs::create_dir(fixture.src().join("missing")).unwrap();
    fs::write(fixture.src().join("missing/mod.rs"), "").unwrap();
    fs::write(fixture.src().join("missing.rs"), "").unwrap();
    assert!(workspace.module_targets(&fixture.src()).is_err());
}
#[test]
fn respects_visibility_and_protects_dirty_and_readonly_parents() {
    let fixture = Fixture::new();
    fs::write(fixture.src().join("lib.rs"), "pub(super) mod existing;\n").unwrap();
    fs::write(fixture.src().join("existing.rs"), "").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(&mut workspace, &fixture.src(), Kind::Rust, "Second");
    assert!(
        fs::read_to_string(fixture.src().join("lib.rs"))
            .unwrap()
            .contains("pub(super) mod second;")
    );
    workspace.open_file(&fixture.src().join("lib.rs")).unwrap();
    workspace
        .apply(Command::Edit {
            path: fixture.src().join("lib.rs"),
            content: "// unsaved\n".into(),
        })
        .unwrap();
    let target = fixture.target(&workspace, &fixture.src());
    assert!(
        workspace
            .apply(Command::CreateTyped {
                parent: fixture.src(),
                kind: Kind::View,
                name: "Blocked".into(),
                registration: Some(target)
            })
            .is_err()
    );
    assert!(!fixture.src().join("blocked_view.rs").exists());
    workspace
        .apply(Command::Reload(fixture.src().join("lib.rs")))
        .unwrap();
    let mut permissions = fs::metadata(fixture.src().join("lib.rs"))
        .unwrap()
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(fixture.src().join("lib.rs"), permissions).unwrap();
    let target = fixture.target(&workspace, &fixture.src());
    assert!(
        workspace
            .apply(Command::CreateTyped {
                parent: fixture.src(),
                kind: Kind::View,
                name: "Denied".into(),
                registration: Some(target)
            })
            .is_err()
    );
    assert!(!fixture.src().join("denied_view.rs").exists());
}
#[test]
fn crate_root_ambiguity_requires_explicit_choice_and_custom_targets_work() {
    let fixture = Fixture::new();
    fs::write(fixture.src().join("main.rs"), "fn main() {}\n").unwrap();
    let workspace = fixture.workspace();
    assert_eq!(workspace.module_targets(&fixture.src()).unwrap().len(), 2);
    fs::create_dir(fixture.0.join("library")).unwrap();
    fs::write(fixture.0.join("library/root.rs"), "").unwrap();
    fs::write(fixture.0.join("Cargo.toml"),"[package]\nname=\"custom\"\nversion=\"0.1.0\"\nautobins=false\n[lib]\npath=\"library/root.rs\"\n").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(&mut workspace, &fixture.0, Kind::View, "Custom");
    assert!(fixture.0.join("library/custom_view.rs").exists());
}
#[test]
fn module_rename_updates_declaration_and_modern_submodule_directory() {
    let fixture = Fixture::new();
    fs::write(fixture.src().join("lib.rs"), "// keep\npub mod settings;\n").unwrap();
    fs::write(fixture.src().join("settings.rs"), "pub mod nested;\n").unwrap();
    fs::create_dir(fixture.src().join("settings")).unwrap();
    fs::write(fixture.src().join("settings/nested.rs"), "// nested\n").unwrap();
    let mut workspace = fixture.workspace();
    workspace
        .open_file(&fixture.src().join("settings/nested.rs"))
        .unwrap();
    workspace
        .apply(Command::RenameModuleEntry {
            path: fixture.src().join("settings.rs"),
            name: "preferences.rs".into(),
        })
        .unwrap();
    assert_eq!(
        fs::read_to_string(fixture.src().join("lib.rs")).unwrap(),
        "// keep\npub mod preferences;\n"
    );
    assert!(fixture.src().join("preferences/nested.rs").exists());
    assert_eq!(
        workspace.state.active_file,
        Some(fixture.src().join("preferences/nested.rs"))
    );
}
#[test]
fn generated_templates_compile_against_local_omen() {
    let fixture = Fixture::new();
    let omen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/omen")
        .canonicalize()
        .unwrap();
    fs::write(fixture.0.join("Cargo.toml"),format!("[package]\nname=\"omen_generated_files\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\nomen={{path={:?},default-features=false}}\n",omen.to_string_lossy())).unwrap();
    let mut workspace = fixture.workspace();
    for (kind, name) in [
        (Kind::View, "Dashboard"),
        (Kind::State, "Dashboard"),
        (Kind::Environment, "Dashboard"),
        (Kind::Localization, "Dashboard"),
        (Kind::Module, "Settings"),
        (Kind::Rust, "Helpers"),
    ] {
        fixture.create(&mut workspace, &fixture.src(), kind, name);
    }
    let result = std::process::Command::new("cargo")
        .args(["check", "--offline", "--manifest-path"])
        .arg(fixture.0.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/omen-template-check"),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn palette_and_context_menu_create_a_view_through_the_async_ui_flow() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, Value, View, ViewExt, ViewKind, ViewNode};
    fn labels(node: &ViewNode, out: &mut Vec<String>) {
        if let Some(label) = &node.style.accessibility.label {
            out.push(label.clone());
        }
        match &node.kind {
            ViewKind::Button { label, .. } => out.push(label.clone()),
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                for child in children {
                    labels(child, out)
                }
            }
            _ => {}
        }
    }
    fn find<'a>(node: &'a ViewNode, matches: &impl Fn(&ViewNode) -> bool) -> Option<&'a ViewNode> {
        if matches(node) {
            return Some(node);
        }
        match &node.kind {
            ViewKind::Group(children)
            | ViewKind::Layout(children)
            | ViewKind::List(children)
            | ViewKind::Card(children)
            | ViewKind::Presentation { children, .. } => {
                children.iter().find_map(|node| find(node, matches))
            }
            _ => None,
        }
    }
    fn contains(node: &ViewNode, icon: &str, text: &str) -> (bool, bool) {
        let mut result = (
            matches!(&node.kind,ViewKind::Icon(name) if name==icon),
            matches!(&node.kind,ViewKind::Text(value) if value==text),
        );
        if let ViewKind::Group(children) | ViewKind::Layout(children) = &node.kind {
            for child in children {
                let found = contains(child, icon, text);
                result.0 |= found.0;
                result.1 |= found.1;
            }
        }
        result
    }
    let fixture = Fixture::new();
    let app = omen_studio::app::StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.clone().receive()).unwrap();
    app.send(Command::OpenProject(fixture.0.clone()));
    app.flush();
    pool.run_until_stalled();
    app.show_context(omen_studio::state::TreeRow {
        path: fixture.src(),
        depth: 0,
        directory: true,
        expanded: true,
    });
    let mut runtime = Runtime::new(
        omen_studio::views::Studio {}
            .environment(Value::new(app.clone()))
            .into_node(),
    );
    let mut actions = Vec::new();
    labels(&runtime.render().unwrap(), &mut actions);
    assert!(actions.iter().any(|label| label == "Create New…"));
    for kind in Kind::CREATABLE {
        assert!(!actions.iter().any(|label| label == kind.label()));
    }
    let context = runtime.render().unwrap();
    let ViewKind::Button { action, .. } = &find(
        &context,
        &|node| matches!(&node.kind, ViewKind::Button { label, .. } if label == "Create New…"),
    )
    .unwrap()
    .kind
    else {
        panic!("Create New menu action")
    };
    action.invoke();
    assert!(app.palette_visible.get());
    assert!(!app.context_visible.get());
    let mut actions = Vec::new();
    labels(&runtime.render().unwrap(), &mut actions);
    for kind in Kind::CREATABLE {
        assert!(actions.iter().any(|label| label == kind.label()));
    }
    let chooser = runtime.render().unwrap();
    for label in [
        "Create New",
        "Choose what you'd like to create in your project.",
    ] {
        assert!(
            find(
                &chooser,
                &|node| matches!(&node.kind, ViewKind::Text(text) if text == label)
            )
            .is_some()
        );
    }
    for kind in Kind::CREATABLE {
        let row = find(&chooser, &|node| {
            node.style.accessibility.label.as_deref() == Some(kind.label())
        })
        .unwrap();
        assert_eq!(contains(row, kind.icon(), kind.description()), (true, true));
    }
    find(&chooser, &|node| {
        node.style.accessibility.label.as_deref() == Some(Kind::View.label())
    })
    .unwrap()
    .style
    .on_tap
    .as_ref()
    .unwrap()
    .invoke();
    assert!(!app.palette_visible.get());
    runtime.render().unwrap();
    for job in runtime.take_tasks() {
        pool.spawner().spawn_local(job.run()).unwrap();
    }
    for _ in 0..20 {
        app.flush();
        pool.run_until_stalled();
        if !app.new_entry.get().resolving {
            break;
        }
    }
    assert!(!app.new_entry.get().resolving);
    assert_eq!(app.new_entry.get().target, Some(0));
    app.entry_name().set("Dashboard".into());
    app.submit_new_entry();
    app.flush();
    pool.run_until_stalled();
    assert!(fixture.src().join("dashboard_view.rs").exists());
    assert!(!app.new_entry_visible.get());
    assert_eq!(
        app.workspace.get().active_file,
        Some(fixture.src().join("dashboard_view.rs"))
    );
}

#[test]
fn conditional_modules_report_a_diagnostic_instead_of_editing_declarations() {
    let fixture = Fixture::new();
    fs::write(
        fixture.src().join("lib.rs"),
        "#[path=\"custom.rs\"] pub mod settings;\n",
    )
    .unwrap();
    fs::write(fixture.src().join("custom.rs"), "").unwrap();
    let mut workspace = fixture.workspace();
    assert!(
        workspace
            .affected_declarations(&fixture.src().join("custom.rs"))
            .is_err()
    );
    assert!(
        workspace
            .apply(Command::RenameModuleEntry {
                path: fixture.src().join("custom.rs"),
                name: "new.rs".into()
            })
            .is_err()
    );
    assert!(fixture.src().join("custom.rs").exists());
    assert!(!fixture.src().join("new.rs").exists());
}

#[test]
fn modern_module_folder_rename_moves_the_owner_and_delete_only_affects_children() {
    let fixture = Fixture::new();
    fs::write(fixture.src().join("lib.rs"), "pub mod settings;\n").unwrap();
    fs::write(fixture.src().join("settings.rs"), "pub mod nested;\n").unwrap();
    fs::create_dir(fixture.src().join("settings")).unwrap();
    fs::write(fixture.src().join("settings/nested.rs"), "").unwrap();
    let mut workspace = fixture.workspace();
    let deletes = workspace
        .affected_declarations(&fixture.src().join("settings"))
        .unwrap();
    assert_eq!(deletes.len(), 1);
    assert_eq!(deletes[0].name, "nested");
    assert_eq!(deletes[0].file, fixture.src().join("settings.rs"));
    let renames = workspace
        .affected_rename_declarations(&fixture.src().join("settings"))
        .unwrap();
    assert_eq!(renames[0].name, "settings");
    workspace
        .apply(Command::RenameModuleEntry {
            path: fixture.src().join("settings"),
            name: "preferences".into(),
        })
        .unwrap();
    assert!(fixture.src().join("preferences.rs").exists());
    assert!(fixture.src().join("preferences/nested.rs").exists());
    assert!(!fixture.src().join("settings.rs").exists());
    assert!(
        fs::read_to_string(fixture.src().join("lib.rs"))
            .unwrap()
            .contains("pub mod preferences;")
    );
}
#[test]
fn insertion_preserves_comments_attributes_crlf_and_detects_raw_duplicates() {
    let fixture = Fixture::new();
    fs::write(
        fixture.src().join("lib.rs"),
        "#![allow(dead_code)]\r\n// existing module\r\nmod r#existing;\r\n",
    )
    .unwrap();
    fs::write(fixture.src().join("existing.rs"), "").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(&mut workspace, &fixture.src(), Kind::Rust, "Added");
    let source = fs::read_to_string(fixture.src().join("lib.rs")).unwrap();
    assert!(
        source.starts_with("#![allow(dead_code)]\r\n// existing module\r\nmod r#existing;\r\n")
    );
    assert!(source.ends_with("mod added;\r\n"));
    assert!(!source.replace("\r\n", "").contains('\n'));
    assert!(
        omen_studio::workspace::modules::insertion(
            &source,
            &fixture.target(&workspace, &fixture.src()),
            "existing"
        )
        .is_err()
    );
}

#[test]
fn source_without_a_trailing_newline_keeps_new_declaration_outside_comment() {
    let fixture = Fixture::new();
    fs::write(fixture.src().join("lib.rs"), "//! module documentation").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(&mut workspace, &fixture.src(), Kind::View, "Home");
    let source = fs::read_to_string(fixture.src().join("lib.rs")).unwrap();
    assert_eq!(source, "//! module documentation\npub mod home_view;\n");
    assert!(
        syn::parse_file(&source)
            .unwrap()
            .items
            .iter()
            .any(|item| matches!(item, syn::Item::Mod(_)))
    );
}
#[cfg(unix)]
#[test]
fn creation_refuses_a_module_subdirectory_symlink_outside_the_project() {
    let fixture = Fixture::new();
    let outside = fixture.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(fixture.src().join("lib.rs"), "mod linked;\n").unwrap();
    fs::write(fixture.src().join("linked.rs"), "").unwrap();
    std::os::unix::fs::symlink(&outside, fixture.src().join("linked")).unwrap();
    let mut workspace = fixture.workspace();
    let parent = fixture.src().join("linked.rs");
    let target = fixture.target(&workspace, &parent);
    assert!(
        workspace
            .apply(Command::CreateTyped {
                parent,
                kind: Kind::View,
                name: "Escape".into(),
                registration: Some(target)
            })
            .is_err()
    );
    assert!(!outside.join("escape_view.rs").exists());
}

#[test]
fn creation_in_views_is_not_blocked_by_unfinished_main_or_sibling_code() {
    let fixture = Fixture::new();
    let broken = "use omen::{main, view};\nmod views;\nmod unfinished;\nview! { Home { Layout { Text(\"Hello\").color(.white\n } } }\n";
    fs::write(fixture.src().join("main.rs"), broken).unwrap();
    fs::write(fixture.src().join("unfinished.rs"), "fn unfinished( {\n").unwrap();
    fs::create_dir(fixture.src().join("views")).unwrap();
    fs::write(fixture.src().join("views/mod.rs"), "// Views\n").unwrap();
    let mut workspace = fixture.workspace();
    fixture.create(
        &mut workspace,
        &fixture.src().join("views"),
        Kind::View,
        "Test",
    );
    assert!(fixture.src().join("views/test_view.rs").is_file());
    assert_eq!(
        fs::read_to_string(fixture.src().join("views/mod.rs")).unwrap(),
        "// Views\npub mod test_view;\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.src().join("main.rs")).unwrap(),
        broken
    );
}
#[test]
fn unfinished_target_parent_is_not_silently_overwritten_and_plain_creation_still_works() {
    let fixture = Fixture::new();
    let broken = "pub fn unfinished( {\n";
    fs::write(fixture.src().join("lib.rs"), broken).unwrap();
    let mut workspace = fixture.workspace();
    let target = fixture.target(&workspace, &fixture.src());
    let result = workspace.apply(Command::CreateTyped {
        parent: fixture.src(),
        kind: Kind::View,
        name: "Safe".into(),
        registration: Some(target),
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("selected parent module")
    );
    assert!(!fixture.src().join("safe_view.rs").exists());
    assert_eq!(
        fs::read_to_string(fixture.src().join("lib.rs")).unwrap(),
        broken
    );
    workspace
        .apply(Command::CreateTyped {
            parent: fixture.src(),
            kind: Kind::View,
            name: "Safe".into(),
            registration: None,
        })
        .unwrap();
    assert!(fixture.src().join("safe_view.rs").exists());
}
#[test]
fn module_recovery_does_not_invent_declarations_from_unclosed_strings_or_comments() {
    let fixture = Fixture::new();
    fs::write(
        fixture.src().join("lib.rs"),
        "const EXAMPLE: &str = r#\"\nmod fake;\n",
    )
    .unwrap();
    fs::create_dir(fixture.src().join("fake")).unwrap();
    fs::write(fixture.src().join("fake/mod.rs"), "").unwrap();
    let workspace = fixture.workspace();
    assert!(
        workspace
            .module_targets(&fixture.src().join("fake"))
            .is_err()
    );
}

#[gpui::test]
fn create_new_supports_keyboard_search_selection_and_cancel(cx: &mut gpui::TestAppContext) {
    use omen::{Value, View, ViewExt};
    use omen_gpui::OmenWindow;
    use omen_studio::{app::StudioEnvironment, views::Studio};

    let fixture = Fixture::new();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    app.open_file_chooser();
    let root = Studio {}.environment(Value::new(app.clone())).into_node();
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(root));
    vcx.run_until_parked();

    vcx.simulate_keystrokes("r u s t");
    vcx.run_until_parked();
    assert_eq!(app.palette_query.get(), "rust");

    vcx.simulate_keystrokes("down up down");
    vcx.run_until_parked();
    assert_eq!(app.palette_index.get(), 1);
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    assert!(!app.palette_visible.get());
    assert_eq!(app.new_entry.get().kind, Some(Kind::Rust));

    vcx.simulate_keystrokes("n o t e s");
    vcx.run_until_parked();
    assert_eq!(app.new_entry.get().name, "notes");
    vcx.simulate_keystrokes("escape");
    vcx.run_until_parked();
    assert!(!app.new_entry_visible.get());

    app.open_file_chooser();
    vcx.run_until_parked();
    vcx.simulate_keystrokes("shift-tab enter");
    vcx.run_until_parked();
    assert!(!app.palette_visible.get());
}
