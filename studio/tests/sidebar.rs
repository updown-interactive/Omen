use omen_studio::{services::project_tools, state::SidebarPane, theme::LayoutPreferences};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omen-sidebar-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn search_includes_unsaved_unicode_and_excludes_build_binary_and_symlink_files() {
    let fixture = Fixture::new();
    let file = fixture.0.join("main.rs");
    fs::write(&file, "disk contents").unwrap();
    fs::create_dir(fixture.0.join("target")).unwrap();
    fs::write(fixture.0.join("target/generated.rs"), "needle").unwrap();
    fs::write(fixture.0.join("binary"), b"needle\0binary").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&file, fixture.0.join("link.rs")).unwrap();
    let text = "first line\n👋 İ Needle here\n";
    let results = project_tools::search(
        fixture.0.clone(),
        "needle".into(),
        false,
        BTreeMap::from([(file.clone(), text.into())]),
    )
    .unwrap();
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].path, file);
    assert_eq!(
        (results.matches[0].line, results.matches[0].character),
        (1, 5)
    );
    let sensitive = project_tools::search(
        fixture.0.clone(),
        "needle".into(),
        true,
        BTreeMap::from([(file, text.into())]),
    )
    .unwrap();
    assert!(sensitive.matches.is_empty());
}
#[test]
fn search_bounds_results() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("many.txt"), "match\n".repeat(1100)).unwrap();
    let results =
        project_tools::search(fixture.0.clone(), "match".into(), true, BTreeMap::new()).unwrap();
    assert_eq!(results.matches.len(), 1000);
    assert!(results.truncated);
}
#[test]
fn git_status_stage_unstage_rename_and_non_repository_states_are_real() {
    let fixture = Fixture::new();
    assert!(!project_tools::status(fixture.0.clone()).unwrap().repository);
    fixture.git(&["init", "--initial-branch=main"]);
    fixture.git(&["config", "user.name", "Studio Test"]);
    fixture.git(&["config", "user.email", "studio-test@example.invalid"]);
    fs::write(fixture.0.join("file with spaces.rs"), "first\n").unwrap();
    let initial = project_tools::status(fixture.0.clone()).unwrap();
    assert_eq!(initial.changes[0].status(), "??");
    let staged = project_tools::stage(fixture.0.clone(), initial.changes[0].clone(), true).unwrap();
    assert!(staged.changes[0].staged());
    let unstaged =
        project_tools::stage(fixture.0.clone(), staged.changes[0].clone(), false).unwrap();
    assert_eq!(unstaged.changes[0].status(), "??");
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "--no-gpg-sign", "-m", "Initial"]);
    fixture.git(&["mv", "file with spaces.rs", "renamed.rs"]);
    let renamed = project_tools::status(fixture.0.clone()).unwrap();
    assert_eq!(renamed.branch, "main");
    assert_eq!(renamed.changes.len(), 1);
    assert_eq!(
        renamed.changes[0].original.as_deref(),
        Some(fixture.0.join("file with spaces.rs").as_path())
    );
    let unstaged =
        project_tools::stage(fixture.0.clone(), renamed.changes[0].clone(), false).unwrap();
    assert!(unstaged.changes.iter().all(|change| !change.staged()));
}
#[test]
fn pane_preferences_default_to_explorer_and_restore_the_selected_pane() {
    let old: LayoutPreferences = serde_json::from_str("{\"sidebar_width\":280}").unwrap();
    assert_eq!(old.active_sidebar, SidebarPane::Explorer);
    let mut layout = old;
    layout.active_sidebar = SidebarPane::Agent;
    let restored: LayoutPreferences =
        serde_json::from_slice(&serde_json::to_vec(&layout).unwrap()).unwrap();
    assert_eq!(restored.active_sidebar, SidebarPane::Agent);
}
fn find<'a>(node: &'a omen::ViewNode, label: &str) -> Option<&'a omen::ViewNode> {
    use omen::ViewKind;
    if node.style.accessibility.label.as_deref() == Some(label)
        || matches!(&node.kind,ViewKind::Text(text)|ViewKind::Button{label:text,..} if text==label)
    {
        return Some(node);
    }
    match &node.kind {
        ViewKind::Group(nodes)
        | ViewKind::Layout(nodes)
        | ViewKind::List(nodes)
        | ViewKind::Card(nodes)
        | ViewKind::Presentation {
            children: nodes, ..
        } => nodes.iter().find_map(|node| find(node, label)),
        _ => None,
    }
}
#[test]
fn activity_bar_switches_real_panes_and_preserves_the_editor_and_search_query() {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen::{Runtime, View};
    use omen_studio::{app::StudioEnvironment, workspace::Command};
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[package]\nname=\"panes\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    fs::create_dir(fixture.0.join("src")).unwrap();
    let file = fixture.0.join("src/main.rs");
    fs::write(&file, "fn main() {}\n").unwrap();
    let profile = Fixture::new();
    let app = StudioEnvironment::at(profile.0.clone());
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(fixture.0.clone()));
    app.send(Command::OpenFile(file.clone()));
    app.flush();
    pool.run_until_stalled();
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let first = runtime.render().unwrap();
    assert!(find(&first, "Explorer").is_some());
    for label in [
        "Show Explorer",
        "Show Search",
        "Show Source Control",
        "Show Agent",
    ] {
        assert!(find(&first, label).is_some());
    }
    app.select_sidebar(SidebarPane::Search);
    app.search_query.set("main".into());
    let searching = runtime.render().unwrap();
    assert!(find(&searching, "Search files").is_some());
    futures::executor::block_on(app.search_project());
    assert_eq!(app.search_results.get().matches.len(), 1);
    app.open_search_result(app.search_results.get().matches[0].clone());
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file, Some(file.clone()));
    app.select_sidebar(SidebarPane::SourceControl);
    assert!(find(&runtime.render().unwrap(), "Source Control").is_some());
    let agent = runtime.render().unwrap();
    find(&agent, "Show Agent")
        .unwrap()
        .style
        .on_tap
        .as_ref()
        .unwrap()
        .invoke();
    let agent = runtime.render().unwrap();
    assert!(find(&agent, "Ask Codex about this project.").is_some());
    assert!(find(&agent, "Source Control").is_none());
    assert_eq!(app.workspace.get().active_file.as_ref(), Some(&file));
    app.agent_prompt.set("Review the layout".into());
    app.select_sidebar(SidebarPane::Search);
    assert_eq!(app.search_query.get(), "main");
    assert!(app.editor_document(file).state.get().selection.end > 0);
    assert!(find(&runtime.render().unwrap(), "main.rs").is_some());
    app.select_sidebar(SidebarPane::Agent);
    assert_eq!(app.agent_prompt.get(), "Review the layout");
    app.select_sidebar(SidebarPane::Search);
    app.flush();
    let saved: LayoutPreferences =
        serde_json::from_slice(&fs::read(profile.0.join("layout.json")).unwrap()).unwrap();
    assert_eq!(saved.active_sidebar, SidebarPane::Search);
}
