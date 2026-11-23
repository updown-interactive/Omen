use futures::{executor::LocalPool, task::LocalSpawnExt};
use omen::{EditorEvent, Runtime, View};
use omen_studio::{app::StudioEnvironment, workspace::Command};
use std::fs;
#[test]
fn component_library_filters_documents_and_inserts_at_saved_cursor() {
    let root = std::env::temp_dir().join(format!("omen-library-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("project/src")).unwrap();
    fs::write(
        root.join("project/Cargo.toml"),
        "[package]\nname=\"library\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    let source = "use omen::prelude::*;\nview! { Home {\n    \n} }\n";
    let file = root.join("project/src/main.rs");
    fs::write(&file, source).unwrap();
    let file = file.canonicalize().unwrap();
    let app = StudioEnvironment::at(root.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(root.join("project")));
    app.send(Command::OpenFile(file.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(file.clone());
    let cursor = source.find("    \n").unwrap() + 4;
    document
        .state
        .update(|state| state.selection = cursor..cursor);
    app.open_component_library();
    assert!(app.library_visible.get());
    assert!(
        app.library_items
            .get()
            .iter()
            .any(|item| item.snippet.name == "Gap")
    );
    assert!(
        app.library_items
            .get()
            .iter()
            .all(|item| !item.documentation.is_empty() && !item.summary.is_empty())
    );
    app.library_query.set("button controls".into());
    app.library_index.set(0);
    let mut runtime = Runtime::new(app.editor_view().into_node());
    runtime.render().unwrap();
    app.insert_library_selection();
    assert!(!app.library_visible.get());
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains("    Button(\"Button\") {\n        // Action\n    }")
    );
    let state = document.state.get();
    assert_eq!(state.text.slice(state.selection), "Button");
    assert!(state.snippet_request.is_some());
    assert_eq!(fs::read_to_string(&file).unwrap(), source);
    app.open_component_library();
    app.library_move(isize::MAX);
    let last = app.library_items.get().len() - 1;
    assert_eq!(app.library_index.get(), last);
    app.library_move(-1);
    assert_eq!(app.library_index.get(), last - 1);
    let mut text = document.state.get().text;
    text.replace(0..0, "// External edit\n");
    document.state.update(|state| state.text = text.clone());
    document.emit(EditorEvent::Edited);
    app.insert_library_selection();
    assert!(document.state.get().text.same_snapshot(&text));
    assert!(app.library_message.get().contains("changed"));
    app.library_query.set("no_such_component".into());
    app.library_index.set(0);
    app.insert_library_selection();
    assert!(document.state.get().text.same_snapshot(&text));
    drop(runtime);
    drop(pool);
    drop(app);
    fs::remove_dir_all(root).unwrap();
}
