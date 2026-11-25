use futures::{
    executor::LocalPool,
    task::LocalSpawnExt,
};
use omen::EditorDiagnostic;
use omen_gpui::OmenWindow;
use omen_studio::{app::StudioEnvironment, workspace::Command};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[gpui::test]
fn find_bar_enter_switches_matches_without_losing_focus_and_supports_toggles(
    cx: &mut gpui::TestAppContext,
) {
    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-find-kb-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let file = root.join("main.rs");
    let source = "fn main() {\n    let foo = 1;\n    let Foo = 2;\n    let foobar = foo;\n}\n";
    fs::write(&file, source).unwrap();

    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root));
    app.send(Command::OpenFile(file.clone()));
    app.flush();
    pool.run_until_stalled();

    let document = app.editor_document(file.clone());
    document.state.update(|state| {
        state.selection = 0..0;
        state.selection_request = Some(0..0);
    });

    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    app.focus_editor();
    vcx.run_until_parked();

    // Open Find via Ctrl/Cmd+F
    let mod_prefix = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    vcx.simulate_keystrokes(&format!("{mod_prefix}-f"));
    vcx.run_until_parked();
    assert!(app.find_visible.get());

    // Type "foo" into the focused Find input
    vcx.simulate_input("foo");
    vcx.run_until_parked();
    assert_eq!(app.find_text.get(), "foo");
    // Case-insensitive + substring matches: "foo", "Foo", "foo" in "foobar", "foo"
    assert_eq!(app.find_match_count.get(), 4);
    assert_eq!(document.state.get().search_matches.len(), 4);

    // Pressing Enter in the Find field advances to match 1 and keeps Find focused
    let find_focus_handle = vcx.update(|window, cx| window.focused(cx));
    assert!(find_focus_handle.is_some());

    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    let first = document.state.get().selection.clone();
    assert_eq!(app.find_match_index.get(), Some(1));
    assert_eq!(document.state.get().text.slice(first.clone()), "foo");
    assert_eq!(
        vcx.update(|window, cx| window.focused(cx)),
        find_focus_handle,
        "Find field must retain keyboard focus when pressing Enter"
    );

    // Pressing Enter again switches to the second match ("Foo")
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    let second = document.state.get().selection.clone();
    assert_ne!(second, first);
    assert_eq!(app.find_match_index.get(), Some(2));
    assert_eq!(document.state.get().text.slice(second.clone()), "Foo");
    assert_eq!(
        vcx.update(|window, cx| window.focused(cx)),
        find_focus_handle
    );

    // Shift+Enter switches back to the first match
    vcx.simulate_keystrokes("shift-enter");
    vcx.run_until_parked();
    assert_eq!(document.state.get().selection, first);
    assert_eq!(app.find_match_index.get(), Some(1));

    // Enabling Case Sensitive + Whole Word narrows matches to exact word "foo" (2 occurrences)
    app.find_case_sensitive.set(true);
    app.find_whole_word.set(true);
    app.refresh_find_matches();
    assert_eq!(app.find_match_count.get(), 2);

    // Escape closes FindBar and clears search_matches
    vcx.simulate_keystrokes("escape");
    vcx.run_until_parked();
    assert!(!app.find_visible.get());
    assert!(document.state.get().search_matches.is_empty());
}

#[gpui::test]
fn goto_line_diagnostics_tabs_quick_open_and_zoom_work_with_keyboard(
    cx: &mut gpui::TestAppContext,
) {
    cx.executor().allow_parking();
    let fixture =
        Fixture(std::env::temp_dir().join(format!("omen-editor-kb-{}", std::process::id())));
    let root = fixture.0.join("project");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let alpha = root.join("alpha.rs");
    let beta = root.join("beta.rs");
    let alpha_src = "fn one() {\n    let a = 1;\n    let b = 2;\n}\n";
    fs::write(&alpha, alpha_src).unwrap();
    fs::write(&beta, "fn two() {}\n").unwrap();

    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root.clone()));
    app.send(Command::OpenFile(alpha.clone()));
    app.send(Command::OpenFile(beta.clone()));
    app.flush();
    pool.run_until_stalled();

    // Switch back to alpha via Ctrl+Tab
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    app.focus_editor();
    vcx.run_until_parked();
    assert_eq!(app.workspace.get().active_file.as_ref(), Some(&beta));

    vcx.simulate_keystrokes("ctrl-tab");
    vcx.run_until_parked();
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().active_file.as_ref(), Some(&alpha));

    // Go to Line: ctrl-g -> "3:9" -> Enter
    vcx.simulate_keystrokes("ctrl-g");
    vcx.run_until_parked();
    assert!(app.goto_visible.get());
    vcx.simulate_input("3:9");
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    assert!(!app.goto_visible.get());
    let doc = app.editor_document(alpha.clone());
    let sel = doc.state.get().selection;
    assert_eq!(doc.state.get().text.byte_to_line(sel.end), 2);
    assert_eq!(
        sel.end - doc.state.get().text.line_start(2),
        8
    );

    // Diagnostics F8 / Shift+F8 navigation
    let first_err = alpha_src.find("let a").unwrap()..alpha_src.find("let a").unwrap() + 5;
    let second_err = alpha_src.find("let b").unwrap()..alpha_src.find("let b").unwrap() + 5;
    doc.state.update(|state| {
        state.selection = 0..0;
        state.diagnostics = std::sync::Arc::new(vec![
            EditorDiagnostic {
                range: first_err.clone(),
                message: "unused a".into(),
                error: false,
            },
            EditorDiagnostic {
                range: second_err.clone(),
                message: "unused b".into(),
                error: true,
            },
        ]);
    });
    vcx.simulate_keystrokes("f8");
    vcx.run_until_parked();
    assert_eq!(doc.state.get().selection, first_err);
    vcx.simulate_keystrokes("f8");
    vcx.run_until_parked();
    assert_eq!(doc.state.get().selection, second_err);
    vcx.simulate_keystrokes("shift-f8");
    vcx.run_until_parked();
    assert_eq!(doc.state.get().selection, first_err);

    // Rename dialog prefills symbol under cursor
    let rename_at = alpha_src.find("one").unwrap();
    doc.state.update(|state| state.selection = rename_at..rename_at);
    app.open_rename();
    assert!(app.rename_visible.get());
    assert_eq!(app.rename_text.get(), "one");
    app.rename_visible.set(false);

    // Editor font zoom
    let initial_size = app.settings.get().editor.font_size;
    let mod_prefix = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    vcx.simulate_keystrokes(&format!("{mod_prefix}-="));
    vcx.run_until_parked();
    assert_eq!(app.settings.get().editor.font_size, initial_size + 1);
    vcx.simulate_keystrokes(&format!("{mod_prefix}-0"));
    vcx.run_until_parked();
    assert_eq!(app.settings.get().editor.font_size, 13);

    // Close active tab with Ctrl/Cmd+W
    vcx.simulate_keystrokes(&format!("{mod_prefix}-w"));
    vcx.run_until_parked();
    app.flush();
    pool.run_until_stalled();
    assert_eq!(app.workspace.get().documents.len(), 1);
    assert_eq!(app.workspace.get().active_file.as_ref(), Some(&beta));

    // Quick Open (Ctrl/Cmd+P) finds and opens alpha.rs
    vcx.simulate_keystrokes(&format!("{mod_prefix}-p"));
    vcx.run_until_parked();
    assert!(app.quick_open_visible.get());
    vcx.simulate_input("alpha");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("enter");
    vcx.run_until_parked();
    app.flush();
    pool.run_until_stalled();
    assert!(!app.quick_open_visible.get());
    assert_eq!(app.workspace.get().active_file.as_ref(), Some(&alpha));
}
