//! Domain value objects.

use super::knowledge::KnowledgeEntry;
use serde::{Deserialize, Serialize};

/// Danger level for a config key.
#[derive(Debug, Clone, PartialEq)]
pub enum Danger {
    Safe,
    Caution,
    Dangerous,
}

/// A detected key in the current file with its explanation.
#[derive(Debug, Clone)]
pub struct DetectedKey {
    pub line: usize,
    pub key: String,
    pub value: String,
    pub section: Option<String>,
    pub entry: Option<KnowledgeEntry>,
}

/// A persistent inline comment attached to a line range of a registered file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// Registered entry path (as stored in index.toml, may contain `~`).
    pub path: String,
    /// First covered line, 1-based, inclusive.
    pub start: usize,
    /// Last covered line, 1-based, inclusive.
    pub end: usize,
    /// Comment text.
    pub text: String,
    /// Creation timestamp (ISO8601 UTC).
    pub created: String,
}

impl Annotation {
    /// Returns true if `line` (1-based) falls within this annotation's range.
    pub fn covers(&self, line: usize) -> bool {
        line >= self.start && line <= self.end
    }
}

/// File metadata as observed through the `Filesystem` port.
#[derive(Debug, Clone)]
pub struct FileMeta {
    pub len: u64,
    pub modified: std::time::SystemTime,
    /// Unix permission bits (full `permissions().mode()`; 0 on non-unix).
    pub mode: u32,
}
