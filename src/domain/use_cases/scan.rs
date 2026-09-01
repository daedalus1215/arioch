//! Scan use case — 1:1 move of `Registry::scan_with_config`.
//!
//! Walks `ScanConfig.scan_paths` through the `Filesystem` port and
//! filters with the domain rules. Returns sorted, deduped suggestions.

use std::path::{Path, PathBuf};

use crate::domain::ports::Filesystem;
use crate::domain::rules;
use crate::domain::value::{FileMeta, ScanConfig};

/// Unix file-type bits (a followed `metadata()` carries the full mode,
/// type bits included).
const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;

/// True when `meta` describes a regular file (followed metadata).
fn is_regular(meta: &Option<FileMeta>) -> bool {
    meta.as_ref().map(|m| m.mode & 0o170000 == S_IFREG).unwrap_or(false)
}

/// True when `meta` describes a directory (followed metadata).
fn is_dir(meta: &Option<FileMeta>) -> bool {
    meta.as_ref().map(|m| m.mode & 0o170000 == S_IFDIR).unwrap_or(false)
}

pub fn scan_for_suggestions(fs: &dyn Filesystem, config: &ScanConfig) -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();

    // Expand exclude paths
    let excludes: Vec<String> = config
        .exclude_paths
        .iter()
        .map(|p| p.replace("~", &home))
        .collect();

    let mut suggestions: Vec<PathBuf> = Vec::new();

    for scan_path in &config.scan_paths {
        let expanded = scan_path.replace("~", &home);
        let path = PathBuf::from(expanded);
        let meta = fs.metadata(&path).ok();

        if is_regular(&meta) {
            if !rules::is_excluded(&path, &excludes)
                && rules::matches_pattern(&path, &config.scan_patterns)
            {
                suggestions.push(path);
            }
        } else if is_dir(&meta) {
            for entry in walkdir(fs, &path, config.scan_depth) {
                if is_regular(&fs.metadata(&entry).ok())
                    && !rules::is_excluded(&entry, &excludes)
                    && rules::matches_pattern(&entry, &config.scan_patterns)
                {
                    suggestions.push(entry);
                }
            }
        }
    }

    suggestions.sort();
    suggestions.dedup();
    suggestions
}

/// Depth-limited walk — 1:1 move of `walkdir_simple_depth` with the
/// `std::fs` calls replaced by port calls.
fn walkdir(fs: &dyn Filesystem, dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut result: Vec<PathBuf> = Vec::new();
    walk(fs, dir, &mut result, 0, max_depth);
    result
}

fn walk(
    fs: &dyn Filesystem,
    dir: &Path,
    result: &mut Vec<PathBuf>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = fs.read_dir(dir) {
        for path in entries {
            result.push(path.clone());
            if is_dir(&fs.metadata(&path).ok()) && !fs.is_symlink(&path) {
                walk(fs, &path, result, depth + 1, max_depth);
            }
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────
// The fake carries unix file-type bits (which `MemFs` cannot express) and
// lists only immediate children, like `std::fs::read_dir`.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io;

    struct FakeFs {
        kinds: BTreeMap<PathBuf, u32>,
    }

    impl FakeFs {
        fn new() -> Self {
            Self {
                kinds: BTreeMap::new(),
            }
        }
        fn file(mut self, p: &str) -> Self {
            self.kinds.insert(p.into(), S_IFREG | 0o644);
            self
        }
        fn dir(mut self, p: &str) -> Self {
            self.kinds.insert(p.into(), S_IFDIR | 0o755);
            self
        }
        fn symlink(mut self, p: &str) -> Self {
            self.kinds.insert(p.into(), 0o120777);
            self
        }
    }

    impl Filesystem for FakeFs {
        fn read_to_string(&self, _path: &Path) -> io::Result<String> {
            Err(io::ErrorKind::NotFound.into())
        }
        fn write(&self, _path: &Path, _contents: &str) -> io::Result<()> {
            Ok(())
        }
        fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
            match self.kinds.get(path) {
                Some(mode) => Ok(FileMeta {
                    len: 0,
                    modified: std::time::UNIX_EPOCH,
                    mode: *mode,
                }),
                None => Err(io::ErrorKind::NotFound.into()),
            }
        }
        fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
            let prefix = path.to_path_buf();
            let depth = prefix.components().count();
            let mut out: Vec<PathBuf> = self
                .kinds
                .keys()
                .filter(|p| p.starts_with(&prefix) && p.components().count() == depth + 1)
                .cloned()
                .collect();
            out.sort();
            Ok(out)
        }
        fn is_symlink(&self, path: &Path) -> bool {
            self.kinds
                .get(path)
                .map(|m| m & 0o170000 == 0o120000)
                .unwrap_or(false)
        }
    }

    fn config(scan_paths: Vec<&str>, exclude_paths: Vec<&str>, patterns: Vec<&str>, depth: usize) -> ScanConfig {
        ScanConfig {
            scan_paths: scan_paths.into_iter().map(String::from).collect(),
            exclude_paths: exclude_paths.into_iter().map(String::from).collect(),
            scan_patterns: patterns.into_iter().map(String::from).collect(),
            scan_depth: depth,
        }
    }

    #[test]
    fn scans_matching_files_in_direct_scan_paths() {
        let fs = FakeFs::new()
            .dir("/h/.ssh")
            .file("/h/.ssh/id_ed25519")
            .file("/h/.ssh/notes.txt");
        let out = scan_for_suggestions(
            &fs,
            &config(vec!["/h/.ssh"], vec![], vec!["id_*"], 3),
        );
        assert_eq!(out, vec![PathBuf::from("/h/.ssh/id_ed25519")]);
    }

    #[test]
    fn direct_file_scan_paths_are_matched_too() {
        let fs = FakeFs::new().file("/etc/ssh/ssh_host_key");
        let out = scan_for_suggestions(
            &fs,
            &config(vec!["/etc/ssh/ssh_host_key"], vec![], vec!["*_key"], 3),
        );
        assert_eq!(out, vec![PathBuf::from("/etc/ssh/ssh_host_key")]);
    }

    #[test]
    fn exclude_paths_filter_by_string_prefix() {
        let fs = FakeFs::new()
            .dir("/etc/ssl")
            .file("/etc/ssl/cert.pem")
            .dir("/etc/ssh")
            .file("/etc/ssh/key.pem");
        let out = scan_for_suggestions(
            &fs,
            &config(vec!["/etc/ssl", "/etc/ssh"], vec!["/etc/ssl"], vec!["*.pem"], 3),
        );
        assert_eq!(out, vec![PathBuf::from("/etc/ssh/key.pem")]);
    }

    #[test]
    fn walk_respects_max_depth() {
        // Levels below /h: a (1), a/b (2), a/b/c (3), a/b/c/d (4).
        // max_depth 3 enumerates levels 1..=4; level-5 files are not seen.
        let fs = FakeFs::new()
            .dir("/h")
            .dir("/h/a")
            .dir("/h/a/b")
            .dir("/h/a/b/c")
            .dir("/h/a/b/c/d")
            .file("/h/a/id_1")
            .file("/h/a/b/c/id_3")
            .file("/h/a/b/c/d/id_4");
        let out = scan_for_suggestions(&fs, &config(vec!["/h"], vec![], vec!["id_*"], 3));
        assert_eq!(
            out,
            vec![PathBuf::from("/h/a/b/c/id_3"), PathBuf::from("/h/a/id_1")]
        );
    }

    #[test]
    fn does_not_recurse_into_symlinked_dirs() {
        // /h/link is a symlink; /h/link/target lives under it.
        let fs = FakeFs::new()
            .dir("/h")
            .symlink("/h/link")
            .dir("/h/link/target")
            .file("/h/link/target/id_hidden")
            .file("/h/id_visible");
        let out = scan_for_suggestions(&fs, &config(vec!["/h"], vec![], vec!["id_*"], 3));
        assert_eq!(out, vec![PathBuf::from("/h/id_visible")]);
    }

    #[test]
    fn results_are_sorted_and_deduped() {
        let fs = FakeFs::new()
            .dir("/h")
            .file("/h/id_a")
            .file("/h/id_b");
        let out = scan_for_suggestions(
            &fs,
            &config(vec!["/h", "/h"], vec![], vec!["id_*"], 3),
        );
        assert_eq!(
            out,
            vec![PathBuf::from("/h/id_a"), PathBuf::from("/h/id_b")]
        );
    }
}
