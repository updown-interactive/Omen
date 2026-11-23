//! Rust's standard module layout, resolved from Cargo targets and syntax trees.
use super::*;
use std::{collections::BTreeSet, ops::Range};
use syn::spanned::Spanned;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleTarget {
    pub file: PathBuf,
    pub inline: Vec<String>,
    pub directory: PathBuf,
}
impl ModuleTarget {
    pub fn label(&self) -> String {
        format!(
            "{} :: {}",
            self.file.display(),
            if self.inline.is_empty() {
                "crate/file module".into()
            } else {
                self.inline.join("::")
            }
        )
    }
}
#[derive(Clone, Debug)]
pub struct Declaration {
    pub file: PathBuf,
    pub name: String,
    pub name_range: Range<usize>,
    pub range: Range<usize>,
}
#[derive(Clone, Debug)]
pub struct ModuleNode {
    pub target: ModuleTarget,
    pub declaration: Option<Declaration>,
}
impl Workspace {
    pub fn module_graph(&self, selected: &Path) -> io::Result<Vec<ModuleNode>> {
        self.module_graph_mode(selected, false)
    }
    fn module_graph_mode(&self, selected: &Path, recover: bool) -> io::Result<Vec<ModuleNode>> {
        let selected = self.project_path(selected)?;
        let root = self
            .state
            .root
            .as_ref()
            .ok_or_else(|| io::Error::other("Open a project first"))?;
        let start = if selected.is_dir() {
            selected.as_path()
        } else {
            selected.parent().unwrap_or(&selected)
        };
        let manifest = start
            .ancestors()
            .take_while(|path| path.starts_with(root))
            .map(|path| path.join("Cargo.toml"))
            .find(|path| path.is_file())
            .ok_or_else(|| io::Error::other("No Cargo package found for this location"))?;
        let package = manifest
            .parent()
            .ok_or_else(|| io::Error::other("Missing Cargo directory"))?;
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(&manifest)?).map_err(io::Error::other)?;
        let mut roots = BTreeSet::new();
        if let Some(path) = config
            .get("lib")
            .and_then(|lib| lib.get("path"))
            .and_then(toml::Value::as_str)
        {
            roots.insert(package.join(path));
        } else if config
            .get("package")
            .and_then(|p| p.get("autolib"))
            .and_then(toml::Value::as_bool)
            != Some(false)
        {
            roots.insert(package.join("src/lib.rs"));
        }
        if config
            .get("package")
            .and_then(|p| p.get("autobins"))
            .and_then(toml::Value::as_bool)
            != Some(false)
        {
            roots.insert(package.join("src/main.rs"));
            if let Ok(entries) = fs::read_dir(package.join("src/bin")) {
                for entry in entries {
                    let path = entry?.path();
                    if path.extension().is_some_and(|ext| ext == "rs") {
                        roots.insert(path);
                    } else if path.is_dir() {
                        roots.insert(path.join("main.rs"));
                    }
                }
            }
        }
        for section in ["bin", "example", "test", "bench"] {
            if let Some(targets) = config.get(section).and_then(toml::Value::as_array) {
                for target in targets {
                    if let Some(path) = target.get("path").and_then(toml::Value::as_str) {
                        roots.insert(package.join(path));
                    } else if let Some(name) = target.get("name").and_then(toml::Value::as_str) {
                        let directory = if section == "bin" {
                            "src/bin"
                        } else if section == "example" {
                            "examples"
                        } else if section == "test" {
                            "tests"
                        } else {
                            "benches"
                        };
                        roots.insert(package.join(directory).join(format!("{name}.rs")));
                        roots.insert(package.join(directory).join(name).join("main.rs"));
                        if section == "bin"
                            && config
                                .get("package")
                                .and_then(|package| package.get("name"))
                                .and_then(toml::Value::as_str)
                                == Some(name)
                        {
                            roots.insert(package.join("src/main.rs"));
                        }
                    }
                }
            }
        }
        match config
            .get("package")
            .and_then(|package| package.get("build"))
        {
            Some(toml::Value::String(path)) => {
                roots.insert(package.join(path));
            }
            Some(toml::Value::Boolean(false)) => {}
            _ => {
                roots.insert(package.join("build.rs"));
            }
        }
        let mut graph = Vec::new();
        let mut visited = BTreeSet::new();
        for file in roots.into_iter().filter(|path| path.is_file()) {
            let file = self.project_path(&file)?;
            let directory = file
                .parent()
                .ok_or_else(|| io::Error::other("Missing source directory"))?
                .to_owned();
            load(
                self,
                ModuleTarget {
                    file,
                    inline: Vec::new(),
                    directory,
                },
                None,
                &mut graph,
                &mut visited,
                recover,
            )?;
        }
        if graph.is_empty() {
            return Err(io::Error::other(
                "No Rust crate roots found; check Cargo.toml target paths",
            ));
        }
        Ok(graph)
    }
    pub fn module_targets(&self, selected: &Path) -> io::Result<Vec<ModuleTarget>> {
        let selected = self.project_path(selected)?;
        let package_root = selected.join("Cargo.toml").is_file();
        let mut targets: Vec<_> = self
            .module_graph_mode(&selected, true)?
            .into_iter()
            .filter(|node| {
                node.target.directory == selected
                    || node.target.file == selected
                    || package_root && node.declaration.is_none() && node.target.inline.is_empty()
            })
            .map(|node| node.target)
            .collect();
        targets.sort_by(|a, b| a.file.cmp(&b.file).then(a.inline.cmp(&b.inline)));
        targets.dedup();
        if targets.is_empty() {
            return Err(io::Error::other(
                "This directory is not a declared Rust module. Create/register its parent module first, or turn off module registration for a plain file.",
            ));
        }
        Ok(targets)
    }
    pub fn modern_owner(&self, path: &Path) -> io::Result<Option<PathBuf>> {
        let path = self.project_path(path)?;
        if !path.is_dir() {
            return Ok(None);
        }
        Ok(self
            .module_graph(&path)?
            .into_iter()
            .find(|node| {
                node.declaration.is_some()
                    && node.target.inline.is_empty()
                    && node.target.directory == path
                    && !node.target.file.starts_with(&path)
            })
            .map(|node| node.target.file))
    }
    pub fn affected_rename_declarations(&self, path: &Path) -> io::Result<Vec<Declaration>> {
        if let Some(owner) = self.modern_owner(path)? {
            self.affected_declarations_mode(&owner, false)
        } else {
            self.affected_declarations_mode(path, false)
        }
    }
    pub fn affected_declarations(&self, path: &Path) -> io::Result<Vec<Declaration>> {
        self.affected_declarations_mode(path, true)
    }
    fn affected_declarations_mode(
        &self,
        path: &Path,
        recover: bool,
    ) -> io::Result<Vec<Declaration>> {
        let path = self.project_path(path)?;
        let graph = self.module_graph_mode(&path, recover)?;
        let files: BTreeSet<_> = graph.iter().map(|node| node.target.file.clone()).collect();
        for file in files {
            let source = self.read_file(&file)?;
            let syntax = match syn::parse_file(&source) {
                Ok(syntax) => syntax,
                Err(_) if recover => recover_module_prefix(&source),
                Err(error) => return Err(io::Error::other(error)),
            };
            if conditional_modules(&syntax.items) {
                return Err(io::Error::other(
                    "Conditional or #[path] modules need manual declaration updates; automatic updates are disabled",
                ));
            }
        }
        let declarations: Vec<_> = graph
            .into_iter()
            .filter_map(|node| {
                let declaration = node.declaration?;
                if node.target.file == path
                    || path.is_dir()
                        && node.target.file.starts_with(&path)
                        && !declaration.file.starts_with(&path)
                {
                    Some(declaration)
                } else {
                    None
                }
            })
            .collect();
        // Discovery can tolerate unfinished siblings, but never edit an
        // incomplete parent or use spans from a recovered parent source.
        for declaration in &declarations {
            syn::parse_file(&self.read_file(&declaration.file)?).map_err(|error| {
                io::Error::other(format!(
                    "Cannot update module declarations in {}: {error}",
                    declaration.file.display()
                ))
            })?;
        }
        Ok(declarations)
    }
}
fn load(
    workspace: &Workspace,
    target: ModuleTarget,
    declaration: Option<Declaration>,
    graph: &mut Vec<ModuleNode>,
    visited: &mut BTreeSet<(PathBuf, Vec<String>)>,
    recover: bool,
) -> io::Result<()> {
    if graph.len() > 20_000 {
        return Err(io::Error::other(
            "Module graph exceeds the 20,000-module limit",
        ));
    }
    if !visited.insert((target.file.clone(), target.inline.clone())) {
        return Ok(());
    }
    let source = workspace.read_file(&target.file)?;
    let syntax = match syn::parse_file(&source) {
        Ok(syntax) => syntax,
        Err(_) if recover => recover_module_prefix(&source),
        Err(error) => {
            return Err(io::Error::other(format!(
                "Cannot resolve modules in {}: {error}",
                target.file.display()
            )));
        }
    };
    let items = scope_items(&syntax.items, &target.inline)?;
    graph.push(ModuleNode {
        target: target.clone(),
        declaration,
    });
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        // Conditional and custom-path modules need an explicit Rust-aware target
        // resolver; do not guess a filesystem destination for them.
        if module.attrs.iter().any(|attr| {
            attr.path().is_ident("path")
                || attr.path().is_ident("cfg")
                || attr.path().is_ident("cfg_attr")
        }) {
            continue;
        }
        let name = module.ident.to_string().trim_start_matches("r#").to_owned();
        let directory = target.directory.join(&name);
        if module.content.is_some() {
            let mut inline = target.inline.clone();
            inline.push(name);
            load(
                workspace,
                ModuleTarget {
                    file: target.file.clone(),
                    inline,
                    directory,
                },
                None,
                graph,
                visited,
                recover,
            )?;
        } else {
            let modern = target.directory.join(format!("{name}.rs"));
            let legacy = directory.join("mod.rs");
            if modern.is_file() && legacy.is_file() {
                return Err(io::Error::other(format!(
                    "Ambiguous Rust module {name}: both {} and {} exist",
                    modern.display(),
                    legacy.display()
                )));
            }
            let file = if modern.is_file() {
                modern
            } else if legacy.is_file() {
                legacy
            } else {
                continue;
            };
            let file = workspace.project_path(&file)?;
            load(
                workspace,
                ModuleTarget {
                    file,
                    inline: Vec::new(),
                    directory,
                },
                Some(Declaration {
                    file: target.file.clone(),
                    name: module.ident.to_string(),
                    name_range: module.ident.span().byte_range(),
                    range: module.span().byte_range(),
                }),
                graph,
                visited,
                recover,
            )?;
        }
    }
    Ok(())
}
pub fn scope_items<'a>(items: &'a [syn::Item], path: &[String]) -> io::Result<&'a [syn::Item]> {
    let Some((first, rest)) = path.split_first() else {
        return Ok(items);
    };
    let module = items
        .iter()
        .find_map(|item| {
            if let syn::Item::Mod(module) = item {
                if module.ident.to_string().trim_start_matches("r#") == first {
                    module.content.as_ref().map(|(_, items)| items)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .ok_or_else(|| io::Error::other("Inline module changed; select the parent module again"))?;
    scope_items(module, rest)
}
pub fn insertion(source: &str, target: &ModuleTarget, name: &str) -> io::Result<String> {
    let syntax = syn::parse_file(source).map_err(|error|io::Error::other(format!("The selected parent module {} has a syntax error: {error}. Fix that file or disable automatic module registration; no files were changed.",target.file.display())))?;
    let items = scope_items(&syntax.items, &target.inline)?;
    if items
        .iter()
        .any(|item| matches!(item,syn::Item::Mod(module) if module.ident.to_string().trim_start_matches("r#")==name))
    {
        return Err(io::Error::other(format!(
            "Module {name} is already declared"
        )));
    }
    let visibility = items
        .iter()
        .find_map(|item| {
            if let syn::Item::Mod(module) = item {
                Some(match &module.vis {
                    syn::Visibility::Inherited => String::new(),
                    visibility => format!("{} ", &source[visibility.span().byte_range()]),
                })
            } else {
                None
            }
        })
        .unwrap_or_else(|| "pub ".into());
    let mut output = source.to_owned();
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if target.inline.is_empty() {
        // Appending within the parsed root scope preserves leading/trailing
        // comments and inner attributes without reattaching them to a new item.
        let separator = if source.is_empty() || source.ends_with('\n') {
            ""
        } else {
            newline
        };
        output.push_str(&format!("{separator}{visibility}mod {name};{newline}"));
    } else {
        let mut scope = &syntax.items;
        let mut closing = 0;
        let mut indent = String::new();
        for part in &target.inline {
            let module = scope
                .iter()
                .find_map(|item| match item {
                    syn::Item::Mod(module)
                        if module.ident.to_string().trim_start_matches("r#") == part =>
                    {
                        Some(module)
                    }
                    _ => None,
                })
                .ok_or_else(|| io::Error::other("Inline module no longer exists"))?;
            let (brace, items) = module
                .content
                .as_ref()
                .ok_or_else(|| io::Error::other("Expected inline module"))?;
            closing = brace.span.close().byte_range().start;
            scope = items;
            let line = &source[..module.ident.span().byte_range().start];
            indent = line
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect();
        }
        output.insert_str(
            closing,
            &format!("{newline}{indent}    {visibility}mod {name};{newline}{indent}"),
        );
    }
    Ok(output)
}

fn conditional_modules(items: &[syn::Item]) -> bool {
    items.iter().any(|item| {
        if let syn::Item::Mod(module) = item {
            module.attrs.iter().any(|attr| {
                attr.path().is_ident("path")
                    || attr.path().is_ident("cfg")
                    || attr.path().is_ident("cfg_attr")
            }) || module
                .content
                .as_ref()
                .is_some_and(|(_, items)| conditional_modules(items))
        } else {
            false
        }
    })
}

// Rust module declarations already completed before an unfinished item remain
// trustworthy. Parse bounded source prefixes instead of treating text inside an
// incomplete macro, comment or string as a declaration. This recovery is used
// for creation/deletion discovery; edited parents and renames remain strict.
fn recover_module_prefix(source: &str) -> syn::File {
    let mut limit = source.len().min(64 * 1024);
    while !source.is_char_boundary(limit) {
        limit -= 1;
    }
    let prefix = &source[..limit];
    let mut ends: Vec<usize> = prefix
        .match_indices('\n')
        .take(256)
        .map(|(offset, _)| offset + 1)
        .chain(
            prefix
                .match_indices(';')
                .take(256)
                .map(|(offset, _)| offset + 1),
        )
        .collect();
    ends.sort_unstable();
    ends.dedup();
    for end in ends.into_iter().rev() {
        if let Ok(syntax) = syn::parse_file(&source[..end]) {
            return syntax;
        }
    }
    syn::File {
        shebang: None,
        attrs: Vec::new(),
        items: Vec::new(),
    }
}
