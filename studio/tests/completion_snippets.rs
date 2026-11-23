//! Type-check generated completion defaults against the real public Omen facade.
use omen_macro_core::editor;
use std::{fs, path::Path};

#[test]
fn generated_component_snippets_compile_against_omen() {
    let root = std::env::temp_dir().join(format!("omen-complete-compile-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    let omen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/omen")
        .canonicalize()
        .unwrap();
    fs::write(root.join("Cargo.toml"),format!("[package]\nname=\"omen_snippet_probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\nomen={{path={:?},default-features=false}}\n",omen.to_string_lossy())).unwrap();
    let declaration = "view! { CustomCard(title: String, count: usize, labels: Vec<String>, visible: Binding<bool>, action: Action) { Text(title) } }";
    let context = format!("{declaration}\nview! {{ Gallery {{\n    \n}} }}");
    let cursor = context.find("    ").unwrap() + 4;
    let snippets = editor::completions(&context, cursor);
    let code = snippets
        .iter()
        .map(|item| item.insert.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!(
        r#"
use omen::prelude::*;
{declaration}
view! {{
    Gallery {{
        @State navigator = Navigator()
        @State selection = "tab"
        @State text = ""
        @State password = ""
        @State checked = false
        @State enabled = false
        @State value = false
        let items = vec!["One".to_string()];
        Layout {{ {code} }}
    }}
}}
fn main() {{ let _ = Gallery {{}}.into_node(); }}
"#
    );
    fs::write(root.join("src/main.rs"), source).unwrap();
    let output = std::process::Command::new("cargo")
        .args(["check", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/omen-template-check"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}
