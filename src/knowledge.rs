//! User knowledge file loading (I/O).
//!
//! The pure knowledge logic — `KnowledgeEntry`, `Danger`, `DetectedKey`,
//! `detect`, `lookup` — lives in `domain::knowledge`. This module keeps the
//! `KnowledgeBase` facade (built-in + user merge) for existing callers.

pub use crate::domain::knowledge::KnowledgeEntry;
pub use crate::domain::value::{Danger, DetectedKey};
use std::path::PathBuf;

/// The knowledge base — built-in + user entries.
pub struct KnowledgeBase {
    entries: Vec<KnowledgeEntry>,
}

impl KnowledgeBase {
    pub fn load() -> Self {
        let mut entries = crate::domain::knowledge::builtin_entries();
        // Load user knowledge file (overrides/augments built-in)
        let user_path = user_knowledge_path();
        if user_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&user_path) {
                if let Ok(user_entries) = parse_user_knowledge(&content) {
                    for ue in user_entries {
                        // User entry overrides built-in with same key
                        if let Some(pos) = entries.iter().position(|e| e.key == ue.key) {
                            entries[pos] = ue;
                        } else {
                            entries.push(ue);
                        }
                    }
                }
            }
        }
        Self { entries }
    }

    /// Detect keys in a file's content based on file type.
    pub fn detect(&self, content: &str, file_type: &str) -> Vec<DetectedKey> {
        crate::domain::knowledge::detect(&self.entries, content, file_type)
    }
}

fn user_knowledge_path() -> PathBuf {
    let mut p = crate::config::active_config_dir();
    p.push("arioch");
    p.push("knowledge.toml");
    p
}

/// Parse user knowledge file (TOML format).
fn parse_user_knowledge(content: &str) -> Result<Vec<KnowledgeEntry>, String> {
    let mut entries = Vec::new();
    let mut current_key = String::new();
    let mut current_what = String::new();
    let mut current_why = String::new();
    let mut current_how = String::new();
    let mut current_danger = Danger::Safe;

    fn flush(
        entries: &mut Vec<KnowledgeEntry>,
        key: &str,
        what: &str,
        why: &str,
        how: &str,
        danger: &Danger,
    ) {
        if !key.is_empty() {
            entries.push(KnowledgeEntry {
                key: key.to_string(),
                what: what.to_string(),
                why: why.to_string(),
                how: how.to_string(),
                danger: danger.clone(),
            });
        }
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[") && trimmed.ends_with(']') {
            // New section = new entry key
            flush(
                &mut entries,
                &current_key,
                &current_what,
                &current_why,
                &current_how,
                &current_danger,
            );
            current_key = trimmed.trim_matches(|c| c == '[' || c == ']').to_string();
            current_what.clear();
            current_why.clear();
            current_how.clear();
            current_danger = Danger::Safe;
        } else if let Some(pos) = trimmed.find('=') {
            let k = trimmed[..pos].trim();
            let v = trimmed[pos + 1..].trim().trim_matches('"');
            match k {
                "what" => current_what = v.to_string(),
                "why" => current_why = v.to_string(),
                "how" => current_how = v.to_string(),
                "danger" => {
                    current_danger = match v {
                        "caution" => Danger::Caution,
                        "dangerous" => Danger::Dangerous,
                        _ => Danger::Safe,
                    }
                }
                _ => {}
            }
        }
    }
    flush(
        &mut entries,
        &current_key,
        &current_what,
        &current_why,
        &current_how,
        &current_danger,
    );

    Ok(entries)
}

// parse_user_knowledge is the I/O-side TOML parser; its tests stay here.
// The detect_* characterization tests live in domain::knowledge.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_knowledge_sections() {
        let content = "[identityfile]\n\
                       what = \"Your key\"\n\
                       danger = \"caution\"\n\
                       \n\
                       [custom_key]\n\
                       what = \"Custom\"\n\
                       why = \"Because\"\n\
                       how = \"Do it\"\n\
                       danger = \"dangerous\"\n";
        let entries = parse_user_knowledge(content).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "identityfile");
        assert_eq!(entries[0].what, "Your key");
        assert_eq!(entries[0].why, "");
        assert_eq!(entries[0].how, "");
        assert_eq!(entries[0].danger, Danger::Caution);
        assert_eq!(entries[1].key, "custom_key");
        assert_eq!(entries[1].what, "Custom");
        assert_eq!(entries[1].why, "Because");
        assert_eq!(entries[1].how, "Do it");
        assert_eq!(entries[1].danger, Danger::Dangerous);
    }

    #[test]
    fn parse_user_knowledge_defaults_danger_to_safe() {
        let content = "[plain]\nwhat = \"x\"\n";
        let entries = parse_user_knowledge(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].danger, Danger::Safe);
    }
}
