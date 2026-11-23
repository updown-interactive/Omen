//! Versioned source inspection; the source buffer remains authoritative.
use std::path::PathBuf;
#[derive(Clone, Debug, Default)]
pub struct InspectorState {
    pub path: Option<PathBuf>,
    pub identity: u64,
    pub revision: u64,
    pub cursor: usize,
    pub node: Option<omen_macro_core::inspector::InspectedNode>,
    pub message: String,
}
