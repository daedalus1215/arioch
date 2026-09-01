//! Real filesystem adapter over `std::fs`.

use std::path::{Path, PathBuf};

use crate::domain::ports::Filesystem;
use crate::domain::value::FileMeta;

pub struct RealFs;

impl Filesystem for RealFs {
    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &str) -> std::io::Result<()> {
        std::fs::write(path, contents)
    }

    fn metadata(&self, path: &Path) -> std::io::Result<FileMeta> {
        let meta = std::fs::metadata(path)?;
        let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        #[cfg(unix)]
        let mode: u32 = {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode: u32 = 0;
        Ok(FileMeta {
            len: meta.len(),
            modified,
            mode,
        })
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<PathBuf>> {
        let entries = std::fs::read_dir(path)?;
        Ok(entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
    }
}
