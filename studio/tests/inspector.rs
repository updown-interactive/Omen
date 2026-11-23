use futures::{
    executor::{LocalPool, block_on},
    task::LocalSpawnExt,
};
use omen::{EditorEvent, Runtime, View};
use omen_studio::{app::StudioEnvironment, workspace::Command};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn inspector_edits_the_real_buffer_saves_and_rejects_stale_selections() {
    let root = std::env::temp_dir().join(format!("omen-inspector-editor-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("project/src")).unwrap();
    let fixture = Fixture(root);
    let project = fixture.0.join("project");
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname=\"inspector\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    let source = "use omen::prelude::*;\nview! { Home { Layout { Text(\"Hi\") }.mode(.vertical).padding(/* keep */24) } }\n";
    let file = project.join("src/main.rs");
    fs::write(&file, source).unwrap();
    let file = file.canonicalize().unwrap();
    let app = StudioEnvironment::at(fixture.0.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(file.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(file.clone());
    let cursor = source.find("Layout").unwrap();
    document
        .state
        .update(|state| state.selection = cursor..cursor);
    block_on(app.inspector_task(
        file.clone(),
        document.state.get().text.clone(),
        cursor,
    )());
    let info = app.inspector.get();
    let node = info.node.unwrap();
    assert_eq!(node.component, "Layout");
    let padding = node
        .properties
        .iter()
        .find(|property| property.name == "padding")
        .unwrap();
    app.inspector_set(&padding.key, "32", false, info.revision);
    app.flush();
    pool.run_until_stalled();
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains("/* keep */32")
    );
    assert!(
        document.state.get().reveal.is_none(),
        "Inspector edits must retain inspector focus"
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), source);
    let updated = document.state.get().text.clone();
    app.inspector_set(&padding.key, "48", false, info.revision);
    assert!(document.state.get().text.same_snapshot(&updated));
    assert!(app.inspector.get().message.contains("changed"));
    let cursor = document.state.get().selection.end;
    block_on(app.inspector_task(file.clone(), updated, cursor)());
    let info = app.inspector.get();
    assert_eq!(
        info.node
            .as_ref()
            .unwrap()
            .properties
            .iter()
            .find(|property| property.name == "padding")
            .unwrap()
            .value,
        "32"
    );
    app.inspector_add("background", info.revision);
    app.flush();
    pool.run_until_stalled();
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains(".background(.surface)")
    );
    app.save_active();
    app.flush();
    pool.run_until_stalled();
    assert!(fs::read_to_string(&file).unwrap().contains("/* keep */32"));
    let mut runtime = Runtime::new(app.editor_view().into_node());
    let rendered = runtime.render().unwrap();
    fn has(node: &omen::ViewNode, text: &str) -> bool {
        if node
            .style
            .text_value
            .as_ref()
            .is_some_and(|value| value.get() == text)
            || matches!(&node.kind,omen::ViewKind::Text(value) if value==text)
            || matches!(&node.kind,omen::ViewKind::Button { label, .. } if label==text)
            || matches!(&node.kind,omen::ViewKind::TextField { value, .. } if value.get()==text)
        {
            return true;
        }
        match &node.kind {
            omen::ViewKind::Group(nodes)
            | omen::ViewKind::Layout(nodes)
            | omen::ViewKind::List(nodes)
            | omen::ViewKind::Card(nodes)
            | omen::ViewKind::Presentation {
                children: nodes, ..
            } => nodes.iter().any(|node| has(node, text)),
            _ => false,
        }
    }
    assert!(has(&rendered, "Inspector"));
    assert!(!has(&rendered, "PREVIEW"));
    // Direct Rust edits are reflected by the next versioned analysis.
    let mut text = document.state.get().text.clone();
    let raw = text.to_string();
    let start = raw.find("/* keep */32").unwrap() + "/* keep */".len();
    text.replace(start..start + 2, "64");
    document.state.update(|state| state.text = text.clone());
    document.emit(EditorEvent::Edited);
    let cursor = document.state.get().selection.end;
    block_on(app.inspector_task(file, text, cursor)());
    assert_eq!(
        app.inspector
            .get()
            .node
            .unwrap()
            .properties
            .iter()
            .find(|p| p.name == "padding")
            .unwrap()
            .value,
        "64"
    );
    let rendered = runtime.render().unwrap();
    assert!(has(&rendered, "Padding"));
    assert!(has(&rendered, "+ Modifier"));
    assert!(has(&rendered, "64"));
    assert!(!has(&rendered, "Apply"));
    assert!(!has(&rendered, "Reset"));
    assert!(has(&rendered, "Vertical"));
    assert!(!has(&rendered, "Edit"));
    assert!(!has(&rendered, "Cancel"));
    input(&rendered, "Padding").unwrap().set("80".into());
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains("/* keep */80")
    );
    let rendered = runtime.render().unwrap();
    input(&rendered, "Padding").unwrap().set("-".into());
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains("/* keep */80")
    );
    let rendered = runtime.render().unwrap();
    input(&rendered, "Padding").unwrap().set("96".into());
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains("/* keep */96")
    );
    let rendered = runtime.render().unwrap();
    button(&rendered, "Horizontal").unwrap().invoke();
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains(".mode(.horizontal)")
    );
    let rendered = runtime.render().unwrap();
    button(&rendered, "Adaptive").unwrap().invoke();
    assert!(
        document
            .state
            .get()
            .text
            .to_string()
            .contains(".direction(.adaptive(compact: .vertical, expanded: .horizontal))")
    );
    let rendered = runtime.render().unwrap();
    assert!(has(&rendered, "Compact"));
    assert!(has(&rendered, "Expanded"));
    app.layout.update(|layout| layout.preview_visible = false);
    app.inspector_focus.set(false);
    app.focus_inspector();
    assert!(!app.inspector_focus.get());
    app.layout.update(|layout| layout.preview_visible = true);
    app.focus_inspector();
    assert!(app.inspector_focus.get());
    app.focus_editor();
    assert!(!app.inspector_focus.get());
    assert!(document.state.get().reveal.is_some());
}

fn input(node: &omen::ViewNode, label: &str) -> Option<omen::Binding<String>> {
    if matches!(&node.kind,omen::ViewKind::TextField { label: name, .. } if name == label) {
        return node.style.text_value.clone();
    }
    match &node.kind {
        omen::ViewKind::Group(nodes)
        | omen::ViewKind::Layout(nodes)
        | omen::ViewKind::List(nodes)
        | omen::ViewKind::Card(nodes)
        | omen::ViewKind::Presentation {
            children: nodes, ..
        } => nodes.iter().find_map(|node| input(node, label)),
        _ => None,
    }
}

fn button(node: &omen::ViewNode, label: &str) -> Option<omen::Action> {
    if let omen::ViewKind::Button {
        label: title,
        action,
    } = &node.kind
        && title == label
    {
        return Some(action.clone());
    }
    match &node.kind {
        omen::ViewKind::Group(nodes)
        | omen::ViewKind::Layout(nodes)
        | omen::ViewKind::List(nodes)
        | omen::ViewKind::Card(nodes)
        | omen::ViewKind::Presentation {
            children: nodes, ..
        } => nodes.iter().find_map(|node| button(node, label)),
        _ => None,
    }
}
