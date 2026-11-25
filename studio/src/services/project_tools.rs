//! Bounded project search and Git commands, executed away from the UI thread.
use crate::state::{GitChange, SearchMatch, SearchResults, SourceControlState};
use omen::TextSnapshot;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

pub async fn background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (reply, answer) = futures::channel::oneshot::channel();
    std::thread::Builder::new()
        .name("omen-project-tools".into())
        .spawn(move || {
            let _ = reply.send(work());
        })
        .map_err(|error| error.to_string())?;
    answer
        .await
        .map_err(|_| "Project tool stopped unexpectedly".to_string())?
}
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_RESULTS: usize = 1000;
fn walk_project_files(root: &Path) -> Result<(PathBuf, Vec<PathBuf>, bool), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut pending = vec![root.clone()];
    let mut files = Vec::new();
    let mut seen = 0;
    let mut truncated = false;
    while let Some(directory) = pending.pop() {
        let mut entries: Vec<_> = fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .take(50_001usize.saturating_sub(seen))
            .collect::<Result<_, _>>()
            .map_err(|e: io::Error| e.to_string())?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            seen += 1;
            if seen > 50_000 {
                truncated = true;
                return Ok((root, files, truncated));
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            if [
                ".git",
                "target",
                "node_modules",
                ".DS_Store",
                ".omen",
                "build",
                "dist",
            ]
            .iter()
            .any(|skip| name == *skip)
            {
                continue;
            }
            let path = entry.path();
            if !path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&root)
            {
                continue;
            }
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            if !kind.is_file() || entry.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE {
                continue;
            }
            files.push(path);
        }
    }
    Ok((root, files, truncated))
}

/// Bounded sorted list of editable files inside the workspace root.
pub fn list_files(root: PathBuf) -> Result<Vec<PathBuf>, String> {
    let (_, mut files, _) = walk_project_files(&root)?;
    files.sort();
    Ok(files)
}

/// Literal search includes current unsaved snapshots, skips generated folders,
/// binary/large files and symlinks, and bounds traversal and result counts.
pub fn search(
    root: PathBuf,
    query: String,
    case_sensitive: bool,
    documents: BTreeMap<PathBuf, TextSnapshot>,
) -> Result<SearchResults, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut result = SearchResults {
        root: Some(root.clone()),
        query: query.clone(),
        case_sensitive,
        ..Default::default()
    };
    if query.is_empty() {
        return Ok(result);
    }
    let needle = if case_sensitive {
        query.clone()
    } else {
        query.to_lowercase()
    };
    let (_, files, truncated) = walk_project_files(&root)?;
    result.truncated = truncated;
    let mut bytes = 0;
    for path in files {
        if bytes > 64 * 1024 * 1024 {
            result.truncated = true;
            return Ok(result);
        }
        let content = if let Some(snapshot) = documents.get(&path) {
            if snapshot.len() as u64 > MAX_FILE {
                continue;
            }
            snapshot.to_string()
        } else {
            let mut data = Vec::new();
            fs::File::open(&path)
                .and_then(|file| file.take(MAX_FILE + 1).read_to_end(&mut data))
                .map_err(|e| e.to_string())?;
            if data.len() as u64 > MAX_FILE || data.contains(&0) {
                continue;
            }
            let Ok(text) = String::from_utf8(data) else {
                continue;
            };
            text
        };
        bytes += content.len();
        for (line, text) in content.lines().enumerate() {
            let haystack = if case_sensitive {
                text.to_string()
            } else {
                text.to_lowercase()
            };
            if let Some(offset) = haystack.find(&needle) {
                // Case-folding can expand Unicode characters: map the folded
                // offset back to the original UTF-16 column used by the editor.
                let character = if case_sensitive {
                    text[..offset].encode_utf16().count()
                } else {
                    let mut folded = 0;
                    let mut column = 0;
                    for ch in text.chars() {
                        if folded >= offset {
                            break;
                        }
                        folded += ch.to_lowercase().collect::<String>().len();
                        column += ch.len_utf16();
                    }
                    column
                };
                result.matches.push(SearchMatch {
                    path: path.clone(),
                    line,
                    character,
                    preview: text.trim().chars().take(220).collect(),
                });
                if result.matches.len() >= MAX_RESULTS {
                    result.truncated = true;
                    return Ok(result);
                }
            }
        }
    }
    Ok(result)
}
fn git(root: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))
}
pub fn status(root: PathBuf) -> Result<SourceControlState, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut state = SourceControlState {
        root: Some(root.clone()),
        ..Default::default()
    };
    let repo = git(&root, &["rev-parse", "--show-toplevel"])?;
    if !repo.status.success() {
        state.message = "This project is not in a Git repository.".into();
        return Ok(state);
    }
    let repo = PathBuf::from(
        String::from_utf8(repo.stdout)
            .map_err(|e| e.to_string())?
            .trim_end_matches('\n'),
    );
    let output = git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    )?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    if output.stdout.len() > 4 * 1024 * 1024 {
        return Err("Git status is too large to display.".into());
    }
    let mut records = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty());
    while let Some(record) = records.next() {
        if record.len() < 4 {
            return Err("Invalid Git status record".into());
        }
        let path = relative_git_path(&repo, &record[3..])?;
        let original = if matches!(record[0], b'R' | b'C') || matches!(record[1], b'R' | b'C') {
            Some(relative_git_path(
                &repo,
                records.next().ok_or("Missing rename source")?,
            )?)
        } else {
            None
        };
        if path.starts_with(&root) {
            state.changes.push(GitChange {
                path,
                original,
                index: record[0] as char,
                worktree: record[1] as char,
            });
        }
    }
    let branch = git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    state.branch = if branch.status.success() {
        String::from_utf8_lossy(&branch.stdout).trim().into()
    } else {
        "Detached HEAD".into()
    };
    state.repository = true;
    Ok(state)
}
fn relative_git_path(repo: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| "Git returned a filename that is not UTF-8")?;
    let path = Path::new(text);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("Git returned an unsafe path".into());
    }
    Ok(repo.join(path))
}
pub fn stage(root: PathBuf, change: GitChange, staged: bool) -> Result<SourceControlState, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    for path in std::iter::once(&change.path).chain(change.original.iter()) {
        if !path.starts_with(&root)
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err("Change is outside the project".into());
        }
    }
    let paths: Vec<_> = std::iter::once(&change.path)
        .chain(change.original.iter())
        .map(|path| path.to_str().ok_or("Filename is not UTF-8"))
        .collect::<Result<_, _>>()?;
    let mut args = if staged {
        vec!["add", "--"]
    } else {
        vec!["restore", "--staged", "--"]
    };
    // Before the first commit there is no HEAD to restore from.
    if !staged
        && !git(&root, &["rev-parse", "--verify", "HEAD"])?
            .status
            .success()
    {
        args = vec!["rm", "--cached", "--ignore-unmatch", "--"];
    }
    args.extend(paths);
    let output = git(&root, &args)?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    status(root)
}
