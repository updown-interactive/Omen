//! rust-analyzer client. Protocol IO, JSON, debouncing and process ownership stay off GPUI.
pub mod protocol;
mod snippets;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use omen::{EditorCompletion, EditorDiagnostic, TextSnapshot};
use protocol::*;
use serde_json::{Value, json};
pub(crate) use snippets::modifier_completions;
use std::{
    collections::HashMap,
    io::{self, BufReader},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum Query {
    Complete,
    Hover,
    Definition,
    Format,
    Rename(String),
    Actions,
    ResolveAction(Value),
    Tokens,
}
impl Query {
    fn method(&self) -> &'static str {
        match self {
            Self::Complete => "textDocument/completion",
            Self::Hover => "textDocument/hover",
            Self::Definition => "textDocument/definition",
            Self::Format => "textDocument/formatting",
            Self::Rename(_) => "textDocument/rename",
            Self::Actions => "textDocument/codeAction",
            Self::ResolveAction(_) => "codeAction/resolve",
            Self::Tokens => "textDocument/semanticTokens/full",
        }
    }
}
pub enum Event {
    Status(String),
    Tokens {
        path: PathBuf,
        identity: u64,
        spans: Vec<omen::SyntaxSpan>,
    },
    Diagnostics {
        path: PathBuf,
        identity: u64,
        diagnostics: Vec<EditorDiagnostic>,
    },
    Completions {
        path: PathBuf,
        identity: u64,
        cursor: usize,
        items: Vec<EditorCompletion>,
    },
    Hover {
        path: PathBuf,
        identity: u64,
        cursor: usize,
        text: String,
    },
    Definition {
        path: PathBuf,
        position: Value,
    },
    Edited {
        path: PathBuf,
        identity: u64,
        text: TextSnapshot,
    },
    WorkspaceEdit {
        edit: Value,
        expected: HashMap<PathBuf, TextSnapshot>,
    },
    Actions {
        path: PathBuf,
        identity: u64,
        items: Vec<Value>,
    },
}
enum Request {
    Document {
        root: PathBuf,
        path: PathBuf,
        text: TextSnapshot,
        cursor: usize,
    },
    Query {
        root: PathBuf,
        path: PathBuf,
        text: TextSnapshot,
        cursor: usize,
        query: Query,
    },
    Close(PathBuf),
    Stop,
}
struct Worker {
    tx: mpsc::Sender<Request>,
    child: Arc<Mutex<Option<Child>>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
    }
}
#[derive(Clone)]
pub struct LanguageService(Arc<Worker>);
impl LanguageService {
    pub fn start() -> io::Result<(Self, UnboundedReceiver<Event>)> {
        let (tx, rx) = mpsc::channel();
        let (events, updates) = unbounded();
        let child = Arc::new(Mutex::new(None));
        let owned = child.clone();
        std::thread::Builder::new()
            .name("omen-rust-analyzer".into())
            .spawn(move || run(rx, events, owned))?;
        Ok((Self(Arc::new(Worker { tx, child })), updates))
    }
    pub fn close(&self, path: PathBuf) {
        let _ = self.0.tx.send(Request::Close(path));
    }
    pub fn document(&self, root: PathBuf, path: PathBuf, text: TextSnapshot, cursor: usize) {
        let _ = self.0.tx.send(Request::Document {
            root,
            path,
            text,
            cursor,
        });
    }
    pub fn query(
        &self,
        root: PathBuf,
        path: PathBuf,
        text: TextSnapshot,
        cursor: usize,
        query: Query,
    ) {
        let _ = self.0.tx.send(Request::Query {
            root,
            path,
            text,
            cursor,
            query,
        });
    }
}
struct OpenDocument {
    text: TextSnapshot,
    version: i64,
}
struct Pending {
    expected: HashMap<PathBuf, TextSnapshot>,
    path: PathBuf,
    text: TextSnapshot,
    cursor: usize,
    query: Query,
}
struct Rpc {
    stdin: ChildStdin,
    messages: mpsc::Receiver<Value>,
    id: u64,
    root: PathBuf,
    documents: HashMap<PathBuf, OpenDocument>,
    pending: HashMap<u64, Pending>,
    token_types: Vec<String>,
    child: Arc<Mutex<Option<Child>>>,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && let Some(mut child) = child.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn analyzer() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("RUST_ANALYZER") {
        return Ok(path.into());
    }
    if Command::new("rust-analyzer")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok("rust-analyzer".into());
    }
    let output = Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    } else {
        Err(io::Error::other(
            "Install rust-analyzer with `rustup component add rust-analyzer`, or set RUST_ANALYZER",
        ))
    }
}
impl Rpc {
    fn start(root: PathBuf, child_slot: Arc<Mutex<Option<Child>>>) -> io::Result<Self> {
        let mut child = Command::new(analyzer()?)
            .current_dir(&root)
            .env("CARGO_NET_OFFLINE", "true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("rust-analyzer stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("rust-analyzer stdout unavailable"))?;
        *child_slot
            .lock()
            .map_err(|_| io::Error::other("Language process lock poisoned"))? = Some(child);
        let (tx, messages) = mpsc::channel();
        std::thread::Builder::new()
            .name("omen-lsp-reader".into())
            .spawn(move || {
                let mut input = BufReader::new(stdout);
                while let Ok(message) = read_message(&mut input) {
                    if tx.send(message).is_err() {
                        break;
                    }
                }
            })?;
        let mut rpc = Self {
            stdin,
            messages,
            id: 1,
            root: root.clone(),
            documents: HashMap::new(),
            pending: HashMap::new(),
            token_types: Vec::new(),
            child: child_slot,
        };
        rpc.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "processId":std::process::id(),"rootUri":uri(&root),"workspaceFolders":[{"uri":uri(&root),"name":root.file_name().unwrap_or_default().to_string_lossy()}],
            "capabilities":{"workspace":{"configuration":true,"applyEdit":false},"textDocument":{"synchronization":{"didSave":true},"semanticTokens":{"requests":{"full":true},"tokenTypes":["keyword","string","character","comment","number","type","struct","enum","interface","trait","typeParameter","function","method","macro","attribute","namespace","variable","parameter","property","enumMember","lifetime","operator","boolean","const","derive"],"tokenModifiers":[],"formats":["relative"]},"publishDiagnostics":{"versionSupport":true},"completion":{"completionItem":{"snippetSupport":false}},"hover":{"contentFormat":["markdown","plaintext"]},"codeAction":{"codeActionLiteralSupport":{"codeActionKind":{"valueSet":["quickfix","refactor","source"]}},"resolveSupport":{"properties":["edit"]}}}},
            "initializationOptions":{"cargo":{"buildScripts":{"enable":false},"allTargets":false},"procMacro":{"enable":false},"checkOnSave":false,"diagnostics":{"experimental":{"enable":true}},"cachePriming":{"enable":false},"numThreads":2}
        } }))?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let message = rpc
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|error| {
                    io::Error::other(format!("rust-analyzer initialization: {error}"))
                })?;
            if message["id"] == 1 && message.get("method").is_none() {
                if !message["error"].is_null() {
                    return Err(io::Error::other(message["error"].to_string()));
                }
                rpc.token_types=message["result"]["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"].as_array().into_iter().flatten().filter_map(|value|value.as_str().map(String::from)).collect();
                break;
            }
            rpc.server_request(&message)?;
        }
        rpc.notify("initialized", json!({}))?;
        Ok(rpc)
    }
    fn send(&mut self, value: Value) -> io::Result<()> {
        write_message(&mut self.stdin, &value)
    }
    fn notify(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}))
    }
    fn server_request(&mut self, message: &Value) -> io::Result<()> {
        if message.get("id").is_some() && message.get("method").is_some() {
            let result = if message["method"] == "workspace/configuration" {
                let config = json!({"cargo":{"buildScripts":{"enable":false},"allTargets":false},"procMacro":{"enable":false},"checkOnSave":false,"diagnostics":{"experimental":{"enable":true}},"cachePriming":{"enable":false},"numThreads":2});
                Value::Array(
                    message["params"]["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|item| {
                            let section = item["section"]
                                .as_str()
                                .unwrap_or("rust-analyzer")
                                .trim_start_matches("rust-analyzer")
                                .trim_start_matches('.');
                            if section.is_empty() {
                                config.clone()
                            } else {
                                config
                                    .pointer(&format!("/{}", section.replace('.', "/")))
                                    .cloned()
                                    .unwrap_or(Value::Null)
                            }
                        })
                        .collect(),
                )
            } else {
                Value::Null
            };
            self.send(json!({"jsonrpc":"2.0","id":message["id"],"result":result}))?;
        }
        Ok(())
    }
    fn sync(&mut self, path: &PathBuf, text: &TextSnapshot) -> io::Result<()> {
        if let Some(open) = self.documents.get(path) {
            if open.text.same_snapshot(text) {
                return Ok(());
            }
            let version = open.version + 1;
            // String serialization is debounced and happens only on this worker.
            self.notify("textDocument/didChange",json!({"textDocument":{"uri":uri(path),"version":version},"contentChanges":[{"text":text.to_string()}]}))?;
            self.documents.insert(
                path.clone(),
                OpenDocument {
                    text: text.clone(),
                    version,
                },
            );
        } else {
            self.notify("textDocument/didOpen",json!({"textDocument":{"uri":uri(path),"languageId":"rust","version":1,"text":text.to_string()}}))?;
            self.documents.insert(
                path.clone(),
                OpenDocument {
                    text: text.clone(),
                    version: 1,
                },
            );
        }
        Ok(())
    }
    fn query(
        &mut self,
        path: PathBuf,
        text: TextSnapshot,
        cursor: usize,
        query: Query,
    ) -> io::Result<()> {
        self.sync(&path, &text)?;
        let obsolete = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.path == path && pending.query.method() == query.method())
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in obsolete {
            self.pending.remove(&id);
            self.notify("$/cancelRequest", json!({"id":id}))?;
        }
        self.id += 1;
        let id = self.id;
        let mut params =
            json!({"textDocument":{"uri":uri(&path)},"position":position(&text,cursor)});
        match &query {
            Query::Tokens => {
                params = json!({"textDocument":{"uri":uri(&path)}});
            }
            Query::Format => {
                params = json!({"textDocument":{"uri":uri(&path)},"options":{"tabSize":4,"insertSpaces":true}});
            }
            Query::Rename(name) => params["newName"] = json!(name),
            Query::Actions => {
                let pos = position(&text, cursor);
                params["range"] = json!({"start":pos,"end":pos});
                params["context"] = json!({"diagnostics":[]});
            }
            Query::ResolveAction(action) => params = action.clone(),
            _ => {}
        }
        self.send(json!({"jsonrpc":"2.0","id":id,"method":query.method(),"params":params}))?;
        self.pending.insert(
            id,
            Pending {
                expected: self
                    .documents
                    .iter()
                    .map(|(path, document)| (path.clone(), document.text.clone()))
                    .collect(),
                path,
                text,
                cursor,
                query,
            },
        );
        Ok(())
    }
    fn message(&mut self, message: Value, events: &UnboundedSender<Event>) -> io::Result<()> {
        self.server_request(&message)?;
        if message["method"] == "textDocument/publishDiagnostics" {
            let params = &message["params"];
            let Some(path) = path(params["uri"].as_str().unwrap_or_default()) else {
                return Ok(());
            };
            if let Some(open) = self.documents.get(&path) {
                if params["version"]
                    .as_i64()
                    .is_some_and(|version| version != open.version)
                {
                    return Ok(());
                }
                let diagnostics = params["diagnostics"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|value| editor_diagnostic(&open.text, value))
                    .collect();
                let _ = events.unbounded_send(Event::Diagnostics {
                    path,
                    identity: open.text.identity(),
                    diagnostics,
                });
            }
        }
        let Some(id) = message["id"].as_u64() else {
            return Ok(());
        };
        let Some(pending) = self.pending.remove(&id) else {
            return Ok(());
        };
        if !message["error"].is_null() {
            if matches!(pending.query, Query::Format) {
                emit_omen_format(events, pending.path.clone(), &pending.text);
            }
            let _ = events.unbounded_send(Event::Status(
                message["error"]["message"]
                    .as_str()
                    .unwrap_or("Language request failed")
                    .into(),
            ));
            return Ok(());
        }
        let result = &message["result"];
        let identity = pending.text.identity();
        let event = match pending.query {
            Query::Tokens => {
                let data = result["data"].as_array();
                let mut row = 0_u64;
                let mut column = 0_u64;
                let mut spans = Vec::new();
                for token in data
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .chunks_exact(5)
                {
                    let delta = token[0].as_u64().unwrap_or(0);
                    row += delta;
                    column = if delta == 0 {
                        column + token[1].as_u64().unwrap_or(0)
                    } else {
                        token[1].as_u64().unwrap_or(0)
                    };
                    let kind = match self
                        .token_types
                        .get(token[3].as_u64().unwrap_or(0) as usize)
                        .map(String::as_str)
                    {
                        Some("keyword") => omen::SyntaxKind::Keyword,
                        Some("string" | "character") => omen::SyntaxKind::String,
                        Some("comment") => omen::SyntaxKind::Comment,
                        Some("number") => omen::SyntaxKind::Number,
                        Some(
                            "type" | "struct" | "enum" | "interface" | "trait" | "typeParameter",
                        ) => omen::SyntaxKind::Type,
                        Some("function" | "method") => omen::SyntaxKind::Function,
                        Some("macro") => omen::SyntaxKind::Macro,
                        Some("attribute" | "derive") => omen::SyntaxKind::Attribute,
                        Some("namespace") => omen::SyntaxKind::Namespace,
                        Some("variable" | "selfKeyword") => omen::SyntaxKind::Variable,
                        Some("parameter") => omen::SyntaxKind::Parameter,
                        Some("property") => omen::SyntaxKind::Property,
                        Some("enumMember" | "const" | "constant" | "static") => {
                            omen::SyntaxKind::Constant
                        }
                        Some("lifetime" | "label") => omen::SyntaxKind::Lifetime,
                        Some("operator") => omen::SyntaxKind::Operator,
                        Some("boolean") => omen::SyntaxKind::Boolean,
                        _ => continue,
                    };
                    let start = offset(&pending.text, &json!({"line":row,"character":column}));
                    let end = offset(
                        &pending.text,
                        &json!({"line":row,"character":column+token[2].as_u64().unwrap_or(0)}),
                    );
                    if end > start {
                        spans.push(omen::SyntaxSpan {
                            range: start..end,
                            kind: if kind == omen::SyntaxKind::Keyword {
                                match pending.text.slice(start..end).as_ref() {
                                    "if" | "else" | "match" | "for" | "while" | "loop" | "in"
                                    | "return" | "break" | "continue" | "yield" | "await" => {
                                        omen::SyntaxKind::ControlFlow
                                    }
                                    "true" | "false" => omen::SyntaxKind::Boolean,
                                    _ => kind,
                                }
                            } else {
                                kind
                            },
                        });
                    }
                }
                let source = pending.text.to_string();
                let modifiers = omen_macro_core::formatter::modifier_ranges(&source);
                spans.retain(|span| {
                    let index = modifiers.partition_point(|range| range.end <= span.range.start);
                    modifiers
                        .get(index)
                        .is_none_or(|range| range.start >= span.range.end)
                });
                spans.extend(modifiers.into_iter().map(|range| omen::SyntaxSpan {
                    range,
                    kind: omen::SyntaxKind::ViewModifier,
                }));
                spans.sort_by_key(|span| span.range.start);
                Event::Tokens {
                    path: pending.path,
                    identity,
                    spans,
                }
            }
            Query::Complete => {
                let values = result.as_array().or_else(|| result["items"].as_array());
                let line_start = pending
                    .text
                    .line_start(pending.text.byte_to_line(pending.cursor));
                let prefix = pending.text.slice(line_start..pending.cursor);
                let prefix_len = prefix
                    .chars()
                    .rev()
                    .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                    .map(char::len_utf8)
                    .sum::<usize>();
                let items = values
                    .into_iter()
                    .flatten()
                    .take(60)
                    .map(|item| {
                        let edit = &item["textEdit"];
                        let range = if edit.get("range").is_some() {
                            range(&pending.text, &edit["range"])
                        } else if edit.get("replace").is_some() {
                            range(&pending.text, &edit["replace"])
                        } else {
                            pending.cursor - prefix_len..pending.cursor
                        };
                        EditorCompletion {
                            label: item["label"].as_str().unwrap_or_default().into(),
                            insert: edit["newText"]
                                .as_str()
                                .or_else(|| item["insertText"].as_str())
                                .or_else(|| item["label"].as_str())
                                .unwrap_or_default()
                                .into(),
                            range,
                            placeholders: Vec::new(),
                            additional_edits: Vec::new(),
                        }
                    })
                    .collect();
                Event::Completions {
                    path: pending.path,
                    identity,
                    cursor: pending.cursor,
                    items,
                }
            }
            Query::Hover => {
                let contents = &result["contents"];
                let markdown = |item: &serde_json::Value| {
                    let value = item
                        .as_str()
                        .or_else(|| item["value"].as_str())
                        .unwrap_or_default();
                    if let Some(language) = item["language"].as_str() {
                        format!("```{language}\n{value}\n```")
                    } else {
                        value.to_owned()
                    }
                };
                let text = if let Some(items) = contents.as_array() {
                    items.iter().map(markdown).collect::<Vec<_>>().join("\n\n")
                } else {
                    markdown(contents)
                };
                Event::Hover {
                    path: pending.path,
                    identity,
                    cursor: pending.cursor,
                    text,
                }
            }
            Query::Definition => {
                let value = result
                    .as_array()
                    .and_then(|items| items.first())
                    .unwrap_or(result);
                let target = value["uri"]
                    .as_str()
                    .or_else(|| value["targetUri"].as_str());
                if let Some(path) = target.and_then(path) {
                    let position =
                        value.get("targetSelectionRange").unwrap_or(&value["range"])["start"]
                            .clone();
                    Event::Definition { path, position }
                } else {
                    Event::Status("No definition found".into())
                }
            }
            Query::Format => Event::Edited {
                path: pending.path,
                identity,
                text: format_omen(&edits(&pending.text, result)),
            },
            Query::Rename(_) => Event::WorkspaceEdit {
                edit: result.clone(),
                expected: pending.expected,
            },
            Query::Actions => Event::Actions {
                path: pending.path,
                identity,
                items: result.as_array().cloned().unwrap_or_default(),
            },
            Query::ResolveAction(_) => {
                if let Some(edit) = result.get("edit") {
                    Event::WorkspaceEdit {
                        edit: edit.clone(),
                        expected: pending.expected,
                    }
                } else {
                    Event::Status(
                        "This code action requires a server command that is not supported yet"
                            .into(),
                    )
                }
            }
        };
        let _ = events.unbounded_send(event);
        Ok(())
    }
}
fn editor_diagnostic(text: &TextSnapshot, value: &Value) -> Option<EditorDiagnostic> {
    let range = range(text, &value["range"]);
    let message = value["message"].as_str().unwrap_or_default();
    if value["code"].as_str() == Some("proc-macro-disabled")
        || message.starts_with("proc-macro expansion is disabled")
    {
        let call = text.slice(range.clone());
        let path = call.split_once('!').map_or(call.as_ref(), |(path, _)| path);
        let name = path.rsplit("::").next().unwrap_or_default().trim();
        if matches!(name, "view" | "main" | "state" | "environment" | "locale") {
            return None;
        }
    }
    Some(EditorDiagnostic {
        range,
        message: message.into(),
        error: value["severity"].as_i64() == Some(1),
    })
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    #[test]
    fn filters_only_the_disabled_expansion_notice_on_omen_macros() {
        let text = TextSnapshot::from("omen::view! {}\nstate! {}\nother! {}");
        let diagnostic = |start, end, code, message, severity| {
            json!({
                "range": {"start": position(&text, start), "end": position(&text, end)},
                "code": code,
                "message": message,
                "severity": severity
            })
        };
        let notice = "proc-macro expansion is disabled";
        assert!(
            editor_diagnostic(&text, &diagnostic(0, 11, "proc-macro-disabled", notice, 2))
                .is_none()
        );
        assert!(editor_diagnostic(&text, &diagnostic(15, 20, "", notice, 2)).is_none());
        assert!(
            editor_diagnostic(&text, &diagnostic(25, 30, "proc-macro-disabled", notice, 2))
                .is_some()
        );
        let error = diagnostic(6, 10, "macro-error", "Invalid Omen view", 1);
        assert!(editor_diagnostic(&text, &error).unwrap().error);
    }
}

fn format_omen(text: &TextSnapshot) -> TextSnapshot {
    let source = text.to_string();
    let formatted = omen_macro_core::formatter::format_document(&source);
    if formatted == source {
        text.clone()
    } else {
        formatted.into()
    }
}
fn emit_omen_format(events: &UnboundedSender<Event>, path: PathBuf, text: &TextSnapshot) {
    let formatted = format_omen(text);
    if !formatted.same_snapshot(text) {
        let _ = events.unbounded_send(Event::Edited {
            path,
            identity: text.identity(),
            text: formatted,
        });
    }
}
fn run(
    requests: mpsc::Receiver<Request>,
    events: UnboundedSender<Event>,
    child: Arc<Mutex<Option<Child>>>,
) {
    let mut rpc: Option<Rpc> = None;
    let mut snippets = snippets::Catalog::default();
    let mut changes: HashMap<PathBuf, (TextSnapshot, usize)> = HashMap::new();
    let mut changed = Instant::now();
    loop {
        match requests.recv_timeout(Duration::from_millis(25)) {
            Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Request::Close(path)) => {
                changes.remove(&path);
                if let Some(client) = rpc.as_mut() {
                    let _ = client.notify(
                        "textDocument/didClose",
                        json!({"textDocument":{"uri":uri(&path)}}),
                    );
                    client.documents.remove(&path);
                    client.pending.retain(|_, pending| pending.path != path);
                }
            }
            Ok(request) => {
                let (root, path, text, cursor, query) = match request {
                    Request::Document {
                        root,
                        path,
                        text,
                        cursor,
                    } => (root, path, text, cursor, None),
                    Request::Query {
                        root,
                        path,
                        text,
                        cursor,
                        query,
                    } => (root, path, text, cursor, Some(query)),
                    Request::Stop | Request::Close(_) => unreachable!(),
                };
                if path.extension().is_none_or(|extension| extension != "rs") {
                    continue;
                }
                if matches!(query, Some(Query::Complete)) {
                    let items = snippets.complete(&root, &text, cursor);
                    if !items.is_empty() {
                        let _ = events.unbounded_send(Event::Completions {
                            path,
                            identity: text.identity(),
                            cursor,
                            items,
                        });
                        continue;
                    }
                }
                if matches!(query, Some(Query::Hover))
                    && let Some(text_help) =
                        super::hover::component_help(&text, cursor, || snippets.views(&root))
                {
                    let _ = events.unbounded_send(Event::Hover {
                        path,
                        identity: text.identity(),
                        cursor,
                        text: text_help,
                    });
                    continue;
                }
                if rpc.as_ref().is_some_and(|rpc| rpc.root != root) {
                    rpc = None;
                    changes.clear();
                }
                if rpc.is_none() {
                    if !root.join("Cargo.toml").is_file() {
                        if matches!(query, Some(Query::Format)) {
                            emit_omen_format(&events, path.clone(), &text);
                            let _ = events.unbounded_send(Event::Status("Omen formatting complete · Rust formatting requires a Cargo project".into()));
                        } else {
                            let _ = events.unbounded_send(Event::Status(
                                "Rust support requires a Cargo project".into(),
                            ));
                        }
                        continue;
                    }
                    let _ = events.unbounded_send(Event::Status("Starting rust-analyzer…".into()));
                    match Rpc::start(root, child.clone()) {
                        Ok(value) => {
                            rpc = Some(value);
                            let _ = events
                                .unbounded_send(Event::Status("rust-analyzer connected".into()));
                        }
                        Err(error) => {
                            if matches!(query, Some(Query::Format)) {
                                emit_omen_format(&events, path.clone(), &text);
                            }
                            let _ = events.unbounded_send(Event::Status(format!(
                                "Rust language support: {error}"
                            )));
                            continue;
                        }
                    }
                }
                if let Some(query) = query {
                    changes.remove(&path);
                    if let Err(error) = rpc.as_mut().unwrap().query(path, text, cursor, query) {
                        let _ = events.unbounded_send(Event::Status(error.to_string()));
                        rpc = None;
                    }
                } else {
                    changes.insert(path, (text, cursor));
                    changed = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(client) = rpc.as_mut() {
            for message in client.messages.try_iter().take(128).collect::<Vec<_>>() {
                if let Err(error) = client.message(message, &events) {
                    let _ = events.unbounded_send(Event::Status(error.to_string()));
                }
            }
            if changed.elapsed() >= Duration::from_millis(200) {
                for (path, (text, cursor)) in changes.drain() {
                    if let Err(error) = client.sync(&path, &text) {
                        let _ = events.unbounded_send(Event::Status(error.to_string()));
                        continue;
                    }
                    let line = text.line(text.byte_to_line(cursor));
                    let column = cursor - text.line_start(text.byte_to_line(cursor));
                    let prefix = &line[..column.min(line.len())];
                    let automatic = prefix.ends_with('.')
                        || prefix.ends_with("::")
                        || prefix
                            .chars()
                            .rev()
                            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                            .count()
                            >= 2;
                    if !client.token_types.is_empty() {
                        let _ = client.query(path.clone(), text.clone(), cursor, Query::Tokens);
                    }
                    if automatic {
                        let items = snippets.complete(&client.root, &text, cursor);
                        if items.is_empty() {
                            let _ = client.query(path, text, cursor, Query::Complete);
                        } else {
                            let _ = events.unbounded_send(Event::Completions {
                                path,
                                identity: text.identity(),
                                cursor,
                                items,
                            });
                        }
                    }
                }
            }
        }
    }
}
