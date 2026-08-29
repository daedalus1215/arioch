use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A persistent inline comment attached to a line range of a registered file.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnnotationsFile {
    pub annotations: Vec<Annotation>,
}

impl AnnotationsFile {
    fn path() -> PathBuf {
        let mut p = crate::config::active_config_dir();
        p.push("arioch");
        p.push("annotations.toml");
        p
    }

    /// Load annotations from disk. Missing or invalid file yields an empty set.
    pub fn load() -> Vec<Annotation> {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<AnnotationsFile>(&content) {
                Ok(file) => file.annotations,
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }

    /// Persist annotations to disk, creating parent directories if needed.
    pub fn save(annotations: &[Annotation]) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = AnnotationsFile {
            annotations: annotations.to_vec(),
        };
        let content = toml::to_string_pretty(&file)
            .map_err(std::io::Error::other)?;
        fs::write(&path, content)
    }
}
