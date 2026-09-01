//! The 9 CLI commands (SPECS.md §2): build domain params, call use-cases,
//! format stdout.
//!
//! The CLI keeps its own shellexpand-based `expand_path` (shellexpand is
//! banned in `domain/`, allowed here).

use crate::config::Config;
use crate::domain::entity::Entry;
use crate::domain::ports::{Filesystem, RegistryStore};
use crate::domain::rules;
use crate::domain::use_cases::{entry, scan};
use crate::domain::value::ScanConfig;

pub fn cmd_list(entries: &[Entry], json: bool) -> anyhow::Result<()> {
    if json {
        let entries: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "category": e.category,
                    "tags": e.tags,
                    "description": e.description,
                    "alias": e.alias,
                    "related": e.related,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        if entries.is_empty() {
            println!("No entries registered.");
            return Ok(());
        }
        for e in entries {
            let tags = if e.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.tags.join(", "))
            };
            let alias = e.alias.as_deref().map(|a| format!(" (alias: {})", a)).unwrap_or_default();
            println!("{}{}{}{}", e.path, tags, alias, if e.category.is_empty() {
                String::new()
            } else {
                format!("  # {}", e.category)
            });
        }
    }
    Ok(())
}

pub fn cmd_add(
    entries: &mut Vec<Entry>,
    path: &str,
    category: Option<String>,
    tags: Option<String>,
    description: Option<String>,
    alias: Option<String>,
    json: bool,
    store: &dyn RegistryStore,
) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    if !expanded.exists() {
        anyhow::bail!("File not found: {}", path);
    }

    let tag_vec: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let entry = Entry {
        path: path.to_string(),
        category: category.unwrap_or_else(|| rules::guess_category_cli(path)),
        tags: tag_vec,
        description: description.unwrap_or_default(),
        alias,
        related: Vec::new(),
    };

    entries.push(entry);
    store.save(entries)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": path,
                "category": entries.last().unwrap().category,
                "tags": entries.last().unwrap().tags,
                "description": entries.last().unwrap().description,
            })
        );
    } else {
        println!("Added: {} ({})", path, entries.last().unwrap().category);
    }
    Ok(())
}

pub fn cmd_remove(
    entries: &mut Vec<Entry>,
    path: &str,
    json: bool,
    store: &dyn RegistryStore,
) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    let idx = entries
        .iter()
        .position(|e| expand_path(&e.path) == expanded || e.path == path);

    match idx {
        Some(i) => match entry::remove_entry(entries, i) {
            Some(removed) => {
                store.save(entries)?;
                if json {
                    println!("{}", serde_json::json!({"removed": removed.path}));
                } else {
                    println!("Removed: {}", removed.path);
                }
            }
            None => anyhow::bail!("Entry not found: {}", path),
        },
        None => anyhow::bail!("Entry not found: {}", path),
    }
    Ok(())
}

pub fn cmd_tag(
    entries: &mut Vec<Entry>,
    path: &str,
    tag: &str,
    json: bool,
    store: &dyn RegistryStore,
) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    let idx = entries
        .iter()
        .position(|e| expand_path(&e.path) == expanded || e.path == path);

    match idx {
        Some(i) => {
            entry::tag_entry(entries, i, tag);
            store.save(entries)?;
            let e = &entries[i];
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "path": e.path,
                        "tags": e.tags,
                    })
                );
            } else {
                println!("Tagged: {} [{}]", e.path, e.tags.join(", "));
            }
        }
        None => anyhow::bail!("Entry not found: {}", path),
    }
    Ok(())
}

pub fn cmd_map(entries: &[Entry], json: bool) -> anyhow::Result<()> {
    if json {
        let nodes: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.alias.as_deref().unwrap_or_else(|| e.path.rsplit('/').next().unwrap_or(&e.path)),
                    "category": e.category,
                    "related": e.related,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else {
        for entry in entries {
            let name = entry
                .alias
                .as_deref()
                .unwrap_or_else(|| entry.path.rsplit('/').next().unwrap_or(&entry.path));
            println!("[{}]", name);
            for rel in &entry.related {
                println!("  ──▶ {}", rel);
            }
        }
    }
    Ok(())
}

pub fn cmd_scan(fs: &dyn Filesystem, config: &Config, json: bool) -> anyhow::Result<()> {
    let scan_config = ScanConfig {
        scan_paths: config.scan_paths.clone(),
        exclude_paths: config.exclude_paths.clone(),
        scan_patterns: config.scan_patterns.clone(),
        scan_depth: config.scan_depth,
    };
    let suggestions = scan::scan_for_suggestions(fs, &scan_config);

    if json {
        let suggestions: Vec<String> = suggestions
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
    } else {
        if suggestions.is_empty() {
            println!("No suggestions found.");
        } else {
            println!("Found {} potential security files:", suggestions.len());
            for s in &suggestions {
                println!("  {}", s.to_string_lossy());
            }
        }
    }
    Ok(())
}

pub fn cmd_export(entries: &[Entry], output: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(entries)?;
    std::fs::write(output, &json)?;
    println!("Exported {} entries to {}", entries.len(), output);
    Ok(())
}

pub fn cmd_import(
    entries: &mut Vec<Entry>,
    file: &str,
    replace: bool,
    store: &dyn RegistryStore,
) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(file)?;
    let imported: Vec<Entry> = serde_json::from_str(&content)?;
    let count = imported.len();

    if replace {
        *entries = imported;
        println!("Replaced index with {} entries from {}", count, file);
    } else {
        // Merge: only add entries that don't already exist (by path)
        let existing: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
        let mut added = 0;
        for entry in imported {
            if !existing.contains(&entry.path) {
                entries.push(entry);
                added += 1;
            }
        }
        println!("Imported {} new entries from {} ({} skipped)", added, file, count - added);
    }

    store.save(entries)?;
    Ok(())
}

pub fn cmd_init(path: &str) -> anyhow::Result<()> {
    let dir = std::path::Path::new(path);
    if dir.exists() {
        anyhow::bail!("Directory already exists: {}", path);
    }
    std::fs::create_dir_all(dir)?;

    let config_content = r#"# arioch config
scan_paths = ["~/.ssh", "~/.config", "/etc/ssh"]
exclude_paths = ["/etc/ssl/certs"]
scan_patterns = ["id_*", "*.pem", "*.crt", "*.key", "config", "*.conf", "*.toml", "*.yaml", "credentials", "*.env"]
scan_depth = 3
max_file_size = 1048576
refresh_interval = 2
# editor = "nvim"
"#;

    let index_content = "# arioch — security file index\n# Managed by arioch. Safe to edit by hand.\n";

    std::fs::write(dir.join("config.toml"), config_content)?;
    std::fs::write(dir.join("index.toml"), index_content)?;

    println!("Initialized arioch config at {}", path);
    println!("  Use with: arioch --config {}", path);
    Ok(())
}

fn expand_path(path: &str) -> std::path::PathBuf {
    let expanded = shellexpand::tilde(path);
    std::path::PathBuf::from(expanded.into_owned())
}

// ─── Characterization tests (Phase 0) ──────────────────────────────────────
// Pin the CLI-side shellexpand-based expand_path. Relocated from main.rs;
// the CLI guess_category variant lives in domain::rules (guess_category_cli)
// with its tests.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_expand_path_uses_shellexpand() {
        assert_eq!(expand_path("/abs/path"), std::path::PathBuf::from("/abs/path"));
        assert_eq!(expand_path("rel/path"), std::path::PathBuf::from("rel/path"));
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(expand_path("~"), std::path::PathBuf::from(&home));
            assert_eq!(expand_path("~/x"), std::path::PathBuf::from(format!("{home}/x")));
        }
    }
}
