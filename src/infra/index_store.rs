//! TOML-backed registry index (`<config-dir>/arioch/index.toml`).
//!
//! The serialization is a 1:1 move of the registry's hand-rolled TOML code
//! (byte-identical output).

use std::path::Path;

use crate::domain::entity::Entry;
use crate::domain::ports::RegistryStore;

pub struct TomlIndex {
    index_path: std::path::PathBuf,
}

impl TomlIndex {
    pub fn new(config_dir: &Path) -> Self {
        let mut p = config_dir.to_path_buf();
        p.push("arioch");
        p.push("index.toml");
        Self { index_path: p }
    }
}

impl RegistryStore for TomlIndex {
    fn load(&self) -> std::io::Result<Vec<Entry>> {
        if !self.index_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.index_path)
            .map_err(|e| std::io::Error::other(format!("Failed to read index: {e}")))?;
        Ok(parse_index_toml(&content))
    }

    fn save(&self, entries: &[Entry]) -> std::io::Result<()> {
        let content = to_index_toml(entries);
        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.index_path, content)
    }
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn to_index_toml(entries: &[Entry]) -> String {
    let mut output = String::new();
    output.push_str("# arioch — security file index\n");
    output.push_str("# Managed by arioch. Safe to edit by hand.\n\n");

    for entry in entries {
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

    output
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

fn parse_index_toml(content: &str) -> Vec<Entry> {
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

    entries
}

// ─── Characterization tests (Phase 0) ──────────────────────────────────────
// Pin the hand-rolled TOML serialization behavior (escaping on write, no
// unescaping on read, stable output format) - 1:1 relocation of the registry
// tests from before the Registry split.

#[cfg(test)]
mod tests {
    use super::*;

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
    fn toml_round_trip_preserves_plain_entries() {
        let mut e1 = entry("/home/u/.ssh/config");
        e1.category = "ssh-keys".into();
        e1.tags = vec!["bastion".into(), "prod".into()];
        e1.description = "Main ssh config".to_string();
        let mut e2 = entry("/etc/ssl/server.pem");
        e2.alias = Some("srv".into());
        e2.related = vec!["/etc/ssl/ca.pem".into()];
        let entries = vec![e1, e2, entry("/plain/path")];

        let parsed = parse_index_toml(&to_index_toml(&entries));
        assert_eq!(parsed, entries);
    }

    #[test]
    fn toml_serialization_escapes_but_parsing_never_unescapes() {
        let mut e = entry("x");
        e.path = r"C:\dir\file".into();
        e.description = "line1\nline2\ttab".into();
        let entries = vec![e];

        let s = to_index_toml(&entries);
        assert!(s.contains(r#"path = "C:\\dir\\file""#));
        assert!(s.contains("description = \"line1\\nline2\\ttab\""));

        // parse side keeps the escape sequences literally (no unescaping today)
        let parsed = parse_index_toml(&s);
        assert_eq!(parsed[0].path, r"C:\\dir\\file");
        assert_eq!(parsed[0].description, "line1\\nline2\\ttab");
    }

    #[test]
    fn toml_output_format_is_stable() {
        let mut e = entry("/a/b");
        e.category = "certs".into();
        assert_eq!(
            to_index_toml(std::slice::from_ref(&e)),
            "# arioch — security file index\n\
             # Managed by arioch. Safe to edit by hand.\n\
             \n\
             [[entry]]\n\
             path = \"/a/b\"\n\
             category = \"certs\"\n\
             \n"
        );
    }
}
