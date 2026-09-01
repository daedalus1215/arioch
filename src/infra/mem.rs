//! In-memory fakes for the ports (cross-cutting/testing.md).
//! Only compiled under `cargo test`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;

use crate::domain::entity::Entry;
use crate::domain::ports::{
    AnnotationStore, AuditLog, Clipboard, Editor, Filesystem, RegistryStore,
};
use crate::domain::value::{Annotation, FileMeta};

/// HashMap-backed filesystem. `write`/`with_file` record contents;
/// `read_dir` lists keys under a prefix (sorted, for determinism).
#[derive(Default)]
pub struct MemFs {
    files: Mutex<HashMap<PathBuf, String>>,
}

impl MemFs {
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.files.lock().insert(path.into(), contents.to_string());
        self
    }

    /// Test helper: read a file's recorded contents.
    pub fn contents(&self, path: &str) -> Option<String> {
        self.files.lock().get(&PathBuf::from(path)).cloned()
    }
}

impl Filesystem for MemFs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.files
            .lock()
            .get(&path.to_path_buf())
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no file: {path:?}")))
    }

    fn write(&self, path: &Path, contents: &str) -> io::Result<()> {
        self.files
            .lock()
            .insert(path.to_path_buf(), contents.to_string());
        Ok(())
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
        let files = self.files.lock();
        match files.get(&path.to_path_buf()) {
            Some(c) => Ok(FileMeta {
                len: c.len() as u64,
                modified: std::time::UNIX_EPOCH,
                mode: 0o644,
            }),
            None => Err(io::Error::new(io::ErrorKind::NotFound, format!("no file: {path:?}"))),
        }
    }

    fn create_dir_all(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().contains_key(&path.to_path_buf())
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let files = self.files.lock();
        let prefix = path.to_path_buf();
        let mut out: Vec<PathBuf> = files
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .cloned()
            .collect();
        out.sort();
        Ok(out)
    }

    fn is_symlink(&self, _path: &Path) -> bool {
        false
    }
}

/// Editor that does nothing and succeeds.
pub struct NoopEditor;

impl Editor for NoopEditor {
    fn launch(&self, _path: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// Clipboard that records what was copied.
#[derive(Default)]
pub struct VecClipboard {
    pub copied: Mutex<Vec<String>>,
}

impl Clipboard for VecClipboard {
    fn copy(&self, text: &str) -> bool {
        self.copied.lock().push(text.to_string());
        true
    }
}

/// Audit log that records lines in memory.
#[derive(Default)]
pub struct MemAuditLog {
    pub lines: Mutex<Vec<String>>,
}

impl AuditLog for MemAuditLog {
    fn append(&self, action: &str, path: &str, details: &str) -> io::Result<()> {
        self.lines.lock().push(format!("{action} {path} {details}"));
        Ok(())
    }

    fn recent(&self, n: usize) -> Vec<String> {
        self.lines
            .lock()
            .iter()
            .rev()
            .take(n)
            .map(|s| s.clone())
            .rev()
            .collect()
    }
}

/// Registry store over a vec.
#[derive(Default)]
pub struct MemRegistryStore {
    pub entries: Mutex<Vec<Entry>>,
}

impl RegistryStore for MemRegistryStore {
    fn load(&self) -> io::Result<Vec<Entry>> {
        Ok(self.entries.lock().clone())
    }

    fn save(&self, entries: &[Entry]) -> io::Result<()> {
        *self.entries.lock() = entries.to_vec();
        Ok(())
    }
}

/// Annotation store over a vec.
#[derive(Default)]
pub struct MemAnnotationStore {
    pub annotations: Mutex<Vec<Annotation>>,
}

impl AnnotationStore for MemAnnotationStore {
    fn load(&self) -> Vec<Annotation> {
        self.annotations.lock().clone()
    }

    fn save(&self, annotations: &[Annotation]) -> io::Result<()> {
        *self.annotations.lock() = annotations.to_vec();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_fs_read_write_roundtrip() {
        let fs = MemFs::default();
        fs.write(Path::new("/a/b"), "hello").unwrap();
        assert_eq!(fs.read_to_string(Path::new("/a/b")).unwrap(), "hello");
        assert!(fs.exists(Path::new("/a/b")));
        assert!(!fs.exists(Path::new("/a/c")));
        assert!(fs.read_to_string(Path::new("/missing")).is_err());
        assert_eq!(fs.metadata(Path::new("/a/b")).unwrap().len, 5);
    }

    #[test]
    fn mem_fs_read_dir_lists_children_sorted() {
        let fs = MemFs::default().with_file("/d/b", "1").with_file("/d/a", "2");
        let entries = fs.read_dir(Path::new("/d")).unwrap();
        assert_eq!(entries, vec![PathBuf::from("/d/a"), PathBuf::from("/d/b")]);
    }

    #[test]
    fn noop_editor_launches_ok() {
        assert!(NoopEditor.launch(Path::new("/x")).is_ok());
    }

    #[test]
    fn vec_clipboard_records_copies() {
        let clip = VecClipboard::default();
        assert!(clip.copy("first"));
        assert!(clip.copy("second"));
        let got = clip.copied.lock().clone();
        assert_eq!(got, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn mem_audit_log_recent_keeps_last_n_in_order() {
        let log = MemAuditLog::default();
        log.append("add", "/a", "").unwrap();
        log.append("tag", "/b", "x").unwrap();
        log.append("remove", "/c", "").unwrap();
        let recent = log.recent(2);
        assert_eq!(recent, vec!["tag /b x".to_string(), "remove /c ".to_string()]);
    }

    fn entry(path: &str) -> Entry {
        Entry {
            path: path.into(),
            category: String::new(),
            tags: Vec::new(),
            description: String::new(),
            alias: None,
            related: Vec::new(),
        }
    }

    #[test]
    fn mem_registry_store_roundtrip() {
        let store = MemRegistryStore::default();
        assert!(store.load().unwrap().is_empty());
        store.save(&[entry("/a")]).unwrap();
        assert_eq!(store.load().unwrap(), vec![entry("/a")]);
    }

    #[test]
    fn mem_annotation_store_roundtrip() {
        let store = MemAnnotationStore::default();
        assert!(store.load().is_empty());
        let ann = Annotation {
            path: "/a".into(),
            start: 1,
            end: 1,
            text: "n".into(),
            created: "2026-01-01T00:00:00Z".into(),
        };
        store.save(&[ann.clone()]).unwrap();
        assert_eq!(store.load(), vec![ann]);
    }
}
