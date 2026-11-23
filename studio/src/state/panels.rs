//! Sidebar selection and background project-tool results.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarPane {
    #[default]
    Explorer,
    Search,
    SourceControl,
    Agent,
}
impl SidebarPane {
    pub const ALL: [Self; 4] = [
        Self::Explorer,
        Self::Search,
        Self::SourceControl,
        Self::Agent,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Explorer => "Explorer",
            Self::Search => "Search",
            Self::SourceControl => "Source Control",
            Self::Agent => "Agent",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Explorer => "files",
            Self::Search => "search",
            Self::SourceControl => "git-branch",
            Self::Agent => "agent-chat",
        }
    }
}
#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line: usize,
    pub character: usize,
    pub preview: String,
}
#[derive(Clone, Debug, Default)]
pub struct SearchResults {
    pub query: String,
    pub case_sensitive: bool,
    pub root: Option<PathBuf>,
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
    pub message: String,
}
#[derive(Clone, Debug)]
pub struct GitChange {
    pub path: PathBuf,
    pub original: Option<PathBuf>,
    pub index: char,
    pub worktree: char,
}
impl GitChange {
    pub fn staged(&self) -> bool {
        !matches!(self.index, ' ' | '?')
    }
    pub fn unstaged(&self) -> bool {
        self.worktree != ' '
    }
    pub fn status(&self) -> String {
        format!("{}{}", self.index, self.worktree)
    }
}
#[derive(Clone, Debug, Default)]
pub struct SourceControlState {
    pub root: Option<PathBuf>,
    pub branch: String,
    pub changes: Vec<GitChange>,
    pub message: String,
    pub repository: bool,
}
