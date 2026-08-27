use anyhow::{Context, Result};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub suggestions: Vec<PathBuf>,
}

impl Registry {
    pub fn config_dir() -> PathBuf {
        crate::config::active_config_dir()
    }

    fn index_path() -> PathBuf {
        let mut p = Self::config_dir();
        p.push("arioch");
        p.push("index.toml");
        p
    }

    pub fn new() -> Self {
        Self::load().unwrap_or_else(|_| Self::empty())
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::index_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = self.to_toml_string()?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let path = Self::index_path();
        if !path.exists() {
            return Ok(Self::empty());
        }
        let content = fs::read_to_string(&path).context("Failed to read index")?;
        Self::parse_toml(&content)
    }

    pub fn add_entry(&mut self, entry: Entry) {
        // Update if path already exists
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == entry.path) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub fn remove_entry(&mut self, index: usize) -> Option<Entry> {
        self.entries.remove(index).into()
    }

    pub fn get_entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    pub fn get_entry_mut(&mut self, index: usize) -> Option<&mut Entry> {
        self.entries.get_mut(index)
    }

    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .entries
            .iter()
            .map(|e| e.category.clone())
            .filter(|c| !c.is_empty())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }

    pub fn entries_in_category(&self, category: &str) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.category == category)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn scan_with_config(
        &mut self,
        scan_paths: &[String],
        scan_patterns: &[String],
        scan_depth: usize,
    ) -> Vec<PathBuf> {
        self.suggestions.clear();
        let home = std::env::var("HOME").unwrap_or_default();

        for scan_path in scan_paths {
            let expanded = scan_path.replace("~", &home);
            let path = PathBuf::from(expanded);

            if path.is_file() {
                if self.matches_pattern_with_config(&path, scan_patterns) {
                    self.suggestions.push(path);
                }
            } else if path.is_dir() {
                for entry in walkdir_simple_depth(&path, scan_depth) {
                    if entry.is_file() && self.matches_pattern_with_config(&entry, scan_patterns) {
                        self.suggestions.push(entry);
                    }
                }
            }
        }

        self.suggestions.sort();
        self.suggestions.dedup();
        self.suggestions.clone()
    }

    fn matches_pattern_with_config(&self, path: &Path, patterns: &[String]) -> bool {
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            for pattern_str in patterns {
                if let Ok(pattern) = Pattern::new(pattern_str) {
                    if pattern.matches(filename) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn to_toml_string(&self) -> Result<String> {
        let mut output = String::new();
        output.push_str("# arioch — security file index\n");
        output.push_str("# Managed by arioch. Safe to edit by hand.\n\n");

        for entry in &self.entries {
            output.push_str("[[entry]]\n");
            output.push_str(&format!("path = \"{}\"\n", escape_toml_string(&entry.path)));

            if !entry.category.is_empty() {
                output.push_str(&format!("category = \"{}\"\n", escape_toml_string(&entry.category)));
            }

            if !entry.tags.is_empty() {
                let tags: Vec<&str> = entry.tags.iter().map(|s| s.as_str()).collect();
                output.push_str(&format!("tags = [{}]\n", tags.join(", ")));
            }

            if !entry.description.is_empty() {
                output.push_str(&format!(
                    "description = \"{}\"\n",
                    escape_toml_string(&entry.description)
                ));
            }

            if let Some(ref alias) = entry.alias {
                output.push_str(&format!("alias = \"{}\"\n", escape_toml_string(alias)));
            }

            if !entry.related.is_empty() {
                let rel: Vec<&str> = entry.related.iter().map(|s| s.as_str()).collect();
                output.push_str(&format!("related = [{}]\n", rel.join(", ")));
            }

            output.push('\n');
        }

        Ok(output)
    }

    fn parse_toml(content: &str) -> Result<Self> {
        let mut entries = Vec::new();
        let mut current: Option<Entry> = None;

        for line in content.lines() {
            let line = line.trim();

            if line == "[[entry]]" {
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                current = Some(Entry {
                    path: String::new(),
                    category: String::new(),
                    tags: Vec::new(),
                    description: String::new(),
                    alias: None,
                    related: Vec::new(),
                });
                continue;
            }

            if let Some(entry) = current.as_mut() {
                if let Some((key, value)) = parse_toml_kv(line) {
                    match key.as_str() {
                        "path" => entry.path = unquote(value.clone()),
                        "category" => entry.category = unquote(value.clone()),
                        "tags" => entry.tags = parse_toml_array(&value),
                        "description" => entry.description = unquote(value.clone()),
                        "alias" => entry.alias = Some(unquote(value.clone())),
                        "related" => entry.related = parse_toml_array(&value),
                        _ => {}
                    }
                }
            }
        }

        if let Some(entry) = current {
            entries.push(entry);
        }

        Ok(Self {
            entries,
            suggestions: Vec::new(),
        })
    }
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn unquote(s: String) -> String {
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s
    }
}

fn parse_toml_kv(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
        return None;
    }

    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() != 2 {
        return None;
    }

    let key = parts[0].trim().to_string();
    let value = parts[1].trim().to_string();
    Some((key, value))
}

fn parse_toml_array(value: &str) -> Vec<String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return vec![unquote(value.to_string())];
    }

    let inner = &value[1..value.len() - 1];
    inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(unquote)
        .collect()
}

fn walkdir_simple_depth(dir: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut result = Vec::new();
    fn walk(dir: &Path, result: &mut Vec<PathBuf>, depth: usize, max_depth: usize) {
        if depth > max_depth {
            return;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                result.push(path.clone());
                if path.is_dir()
                    && !path.symlink_metadata().map(|m| m.is_symlink()).unwrap_or(false)
                {
                    walk(&path, result, depth + 1, max_depth);
                }
            }
        }
    }
    walk(dir, &mut result, 0, max_depth);
    result
}
