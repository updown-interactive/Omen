use omen::TextSnapshot;
use omen_studio::editor::lsp::{Event, LanguageService, Query, protocol};
use std::{
    fs,
    time::{Duration, Instant},
};

#[test]
fn lsp_framing_and_unicode_positions_roundtrip() {
    let text = TextSnapshot::from("fn main() {\n    let 👩‍💻 = 1;\n}\n");
    let byte = text.to_string().find(" =").unwrap();
    let position = protocol::position(&text, byte);
    assert_eq!(protocol::offset(&text, &position), byte);
    let mut frame = Vec::new();
    let message = serde_json::json!({"jsonrpc":"2.0","method":"test","params":{"text":"世界"}});
    protocol::write_message(&mut frame, &message).unwrap();
    assert_eq!(
        protocol::read_message(&mut std::io::Cursor::new(frame)).unwrap(),
        message
    );
    assert!(
        protocol::read_message(&mut std::io::Cursor::new(
            b"Content-Length: 999999999\r\n\r\n"
        ))
        .is_err()
    );
}

#[test]
#[ignore = "requires the official rust-analyzer component; run explicitly for integration verification"]
fn real_rust_analyzer_reports_diagnostics_and_local_completion() {
    let root = std::env::temp_dir().join(format!("omen-real-lsp-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname = \"omen-lsp-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n").unwrap();
    let path = root.join("src/main.rs");
    let source = "fn main() {\n    let hello_world = 42;\n    hel;\n}\n";
    fs::write(&path, source).unwrap();
    let text = TextSnapshot::from(source);
    let cursor = source.find("hel;").unwrap() + 3;
    let (service, mut events) = LanguageService::start().unwrap();
    service.document(root.clone(), path.clone(), text.clone(), cursor);
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut requested = Instant::now() - Duration::from_secs(2);
    let mut diagnostics = false;
    let mut completion = false;
    while Instant::now() < deadline && !(diagnostics && completion) {
        if requested.elapsed() > Duration::from_millis(900) {
            service.query(
                root.clone(),
                path.clone(),
                text.clone(),
                cursor,
                Query::Complete,
            );
            requested = Instant::now();
        }
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Status(message) => println!("{message}"),
                Event::Diagnostics {
                    identity,
                    diagnostics: items,
                    ..
                } => {
                    assert_eq!(identity, text.identity());
                    diagnostics |= !items.is_empty();
                }
                Event::Completions {
                    identity, items, ..
                } => {
                    assert_eq!(identity, text.identity());
                    completion |= items
                        .iter()
                        .any(|item| item.label.starts_with("hello_world"));
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(service);
    let _ = fs::remove_dir_all(root);
    assert!(diagnostics, "rust-analyzer must publish real diagnostics");
    assert!(
        completion,
        "rust-analyzer must complete the local hello_world variable"
    );
}

#[test]
#[ignore = "requires the official rust-analyzer component"]
fn real_rust_analyzer_supports_hover_definition_format_rename_and_tokens() {
    let root = std::env::temp_dir().join(format!("omen-real-lsp-actions-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname = \"omen-lsp-actions\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n").unwrap();
    let path = root.join("src/main.rs");
    let source = "fn greeting()->i32{42}\nfn main(){let _value=greeting();}\n";
    fs::write(&path, source).unwrap();
    let text = TextSnapshot::from(source);
    let cursor = source.rfind("greeting").unwrap() + 2;
    let (service, mut events) = LanguageService::start().unwrap();
    service.document(root.clone(), path.clone(), text.clone(), cursor);
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut last = Instant::now() - Duration::from_secs(2);
    let (mut hover, mut definition, mut formatting, mut rename, mut tokens, mut actions) =
        (false, false, false, false, false, false);
    while Instant::now() < deadline
        && !(hover && definition && formatting && rename && tokens && actions)
    {
        if last.elapsed() > Duration::from_secs(2) {
            for query in [
                Query::Hover,
                Query::Definition,
                Query::Format,
                Query::Rename("greeting_new".into()),
                Query::Tokens,
                Query::Actions,
            ] {
                service.query(root.clone(), path.clone(), text.clone(), cursor, query);
            }
            last = Instant::now();
        }
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Status(message) => println!("{message}"),
                Event::Hover { text, .. } => hover |= text.contains("greeting"),
                Event::Definition {
                    path: target,
                    position,
                } => definition |= target == path && position["line"] == 0,
                Event::Edited {
                    identity,
                    text: formatted,
                    ..
                } => {
                    assert_eq!(identity, text.identity());
                    formatting |= formatted.contains("fn greeting() -> i32");
                }
                Event::WorkspaceEdit { edit, expected } => {
                    assert_eq!(expected.get(&path).unwrap(), &text);
                    rename |= edit.to_string().contains("greeting_new");
                }
                Event::Tokens { spans, .. } => tokens |= !spans.is_empty(),
                Event::Actions { .. } => actions = true,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(service);
    let _ = fs::remove_dir_all(root);
    assert!(hover, "hover");
    assert!(definition, "definition");
    assert!(formatting, "formatting");
    assert!(rename, "rename");
    assert!(tokens, "semantic highlighting");
    assert!(actions, "code-action response");
}

#[test]
fn format_document_formats_omen_even_without_a_cargo_project() {
    use futures::{FutureExt, StreamExt};
    let root = std::env::temp_dir().join(format!("omen-format-no-cargo-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("main.rs");
    let text = TextSnapshot::from(
        "view! { Layout { Text(\"Hello\").fontSize(13).color(.red) }.padding(4) }",
    );
    let (service, mut events) = LanguageService::start().unwrap();
    service.query(root.clone(), path.clone(), text.clone(), 0, Query::Format);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut formatted = None;
    while Instant::now() < deadline {
        if let Some(Some(Event::Edited {
            path: edited_path,
            identity,
            text: result,
        })) = events.next().now_or_never()
        {
            assert_eq!(edited_path, path);
            assert_eq!(identity, text.identity());
            formatted = Some(result.to_string());
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        formatted
            .expect("Omen formatter response")
            .contains("\n        .fontSize(13)\n        .color(.red)")
    );
    drop(service);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn omen_completion_delivers_project_properties_and_editable_placeholders_without_an_lsp() {
    let root = std::env::temp_dir().join(format!("omen-completion-props-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/profile.rs"),
        "view! { pub Profile(title: String) { Text(title) } }",
    )
    .unwrap();
    let source = "view! { Home { Pro";
    let path = root.join("src/main.rs");
    fs::write(&path, source).unwrap();
    let (service, mut events) = LanguageService::start().unwrap();
    service.query(
        root.clone(),
        path,
        source.into(),
        source.len(),
        Query::Complete,
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut result = None;
    while Instant::now() < deadline && result.is_none() {
        while let Ok(event) = events.try_recv() {
            if let Event::Completions { items, .. } = event {
                result = items.into_iter().find(|item| item.label == "Profile");
            }
        }
        if result.is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let completion = result.expect("Omen completion must work before rust-analyzer starts");
    assert_eq!(completion.insert, "Profile(title: \"value\")");
    assert_eq!(
        &completion.insert[completion.placeholders[0].clone()],
        "\"value\""
    );
    drop(service);
    fs::remove_dir_all(root).unwrap();
}

#[gpui::test]
fn typing_a_dot_offers_keyboard_modifier_completion(cx: &mut gpui::TestAppContext) {
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use omen_gpui::OmenWindow;
    use omen_studio::{app::StudioEnvironment, workspace::Command};

    cx.executor().allow_parking();
    let root =
        std::env::temp_dir().join(format!("omen-modifier-completion-{}", std::process::id()));
    fs::create_dir_all(root.join("project")).unwrap();
    let project = root.join("project").canonicalize().unwrap();
    let path = project.join("home_view.rs");
    let source = r#"view! { Home { Text("Title") } }"#;
    fs::write(&path, source).unwrap();
    let app = StudioEnvironment::at(root.join("profile"));
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(app.receive()).unwrap();
    app.send(Command::OpenProject(project));
    app.send(Command::OpenFile(path.clone()));
    app.flush();
    pool.run_until_stalled();
    let document = app.editor_document(path);
    let cursor = source.find(')').unwrap() + 1;
    document.state.update(|state| {
        state.selection = cursor..cursor;
        state.reveal = Some(cursor..cursor);
    });
    let (_, vcx) = cx.add_window_view(|_, _| OmenWindow::new(app.editor_view()));
    vcx.run_until_parked();

    vcx.simulate_input(".");
    vcx.run_until_parked();
    let state = document.state.get();
    assert!(
        state
            .completions
            .iter()
            .any(|item| item.label == ".padding")
    );
    assert!(!state.completions.iter().any(|item| item.label == ".mode"));

    vcx.simulate_input("pad");
    vcx.run_until_parked();
    assert_eq!(document.state.get().completions.len(), 2);
    vcx.simulate_keystrokes("down up enter");
    vcx.run_until_parked();
    let state = document.state.get();
    assert_eq!(
        state.text,
        r#"view! { Home { Text("Title").padding(24) } }"#
    );
    assert_eq!(&*state.text.slice(state.selection), "24");
    vcx.simulate_input("16");
    vcx.simulate_keystrokes("tab");
    vcx.simulate_input(".fi");
    vcx.run_until_parked();
    vcx.simulate_keystrokes("tab");
    vcx.run_until_parked();
    assert_eq!(
        document.state.get().text,
        r#"view! { Home { Text("Title").padding(16).fill() } }"#
    );

    vcx.simulate_input(".");
    vcx.run_until_parked();
    assert!(!document.state.get().completions.is_empty());
    vcx.simulate_keystrokes("escape");
    vcx.run_until_parked();
    assert!(document.state.get().completions.is_empty());
    drop(app);
    drop(pool);
    fs::remove_dir_all(root).unwrap();
}
