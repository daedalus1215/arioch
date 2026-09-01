//! TOML-backed annotation store (`<config-dir>/arioch/annotations.toml`).

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::domain::ports::AnnotationStore;
use crate::domain::value::Annotation;

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    annotations: Vec<Annotation>,
}

pub struct TomlAnnotations {
    path: std::path::PathBuf,
}

impl TomlAnnotations {
    pub fn new(config_dir: &Path) -> Self {
        let mut p = config_dir.to_path_buf();
        p.push("arioch");
        p.push("annotations.toml");
        Self { path: p }
    }
}

impl AnnotationStore for TomlAnnotations {
    fn load(&self) -> Vec<Annotation> {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => match toml::from_str::<File>(&content) {
                Ok(file) => file.annotations,
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }

    fn save(&self, annotations: &[Annotation]) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File {
            annotations: annotations.to_vec(),
        };
        let content = toml::to_string_pretty(&file).map_err(std::io::Error::other)?;
        std::fs::write(&self.path, content)
    }
}
