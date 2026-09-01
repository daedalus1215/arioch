//! I/O ports (SPECS.md §3).
//!
//! Everything I/O-shaped behind a trait; implemented in `infra/`, bound once
//! at the composition root. Signatures are the load-bearing contract.

use std::path::{Path, PathBuf};

use super::entity::Entry;
use super::value::{Annotation, FileMeta};

pub trait Filesystem: Send + Sync {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;
    fn write(&self, path: &Path, contents: &str) -> std::io::Result<()>;
    fn metadata(&self, path: &Path) -> std::io::Result<FileMeta>;
    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>>;
    /// Lstat-style check: true when `path` itself is a symlink (the
    /// followed `metadata()` cannot distinguish a symlinked dir from a
    /// real dir, and the scan walk must not recurse into symlinked dirs).
    fn is_symlink(&self, path: &Path) -> bool;
}

pub trait Editor: Send + Sync {
    /// Launch the user's editor on `path`, blocking until exit.
    /// The TUI disables raw mode before and re-enables after — this port only
    /// spawns the process.
    fn launch(&self, path: &Path) -> std::io::Result<()>;
}

pub trait Clipboard: Send + Sync {
    /// Copy `text`; returns true if a backend (wl-copy/xclip/xsel) succeeded.
    fn copy(&self, text: &str) -> bool;
}

pub trait AuditLog: Send + Sync {
    fn append(&self, action: &str, path: &str, details: &str) -> std::io::Result<()>;
    /// Same order/format as today's read_history (oldest first, last `n`).
    fn recent(&self, n: usize) -> Vec<String>;
}

pub trait RegistryStore: Send + Sync {
    fn load(&self) -> std::io::Result<Vec<Entry>>;
    /// TOML, byte-identical to today.
    fn save(&self, entries: &[Entry]) -> std::io::Result<()>;
}

pub trait AnnotationStore: Send + Sync {
    fn load(&self) -> Vec<Annotation>;
    fn save(&self, annotations: &[Annotation]) -> std::io::Result<()>;
}
