//! Entry use cases — 1:1 moves of the `Registry` mutation methods.
//!
//! Pure on `&mut Vec<Entry>`. The caller persists through the
//! `RegistryStore` port after mutating (mirrors today's mutate-then-save).

use crate::domain::entity::Entry;

/// Insert `entry`, or replace in place an existing entry with the same
/// `path` (position preserved). 1:1 move of `Registry::add_entry`.
pub fn upsert_entry(entries: &mut Vec<Entry>, entry: Entry) {
    if let Some(existing) = entries.iter_mut().find(|e| e.path == entry.path) {
        *existing = entry;
    } else {
        entries.push(entry);
    }
}

/// Remove and return the entry at `index`. 1:1 move of
/// `Registry::remove_entry`.
pub fn remove_entry(entries: &mut Vec<Entry>, index: usize) -> Option<Entry> {
    entries.remove(index).into()
}

/// Add `tag` to the entry at `index` unless it already has it.
pub fn tag_entry(entries: &mut Vec<Entry>, index: usize, tag: &str) {
    if let Some(entry) = entries.get_mut(index) {
        if !entry.tags.iter().any(|t| t == tag) {
            entry.tags.push(tag.to_string());
        }
    }
}

/// Overwrite the category of the entry at `index`.
pub fn set_category(entries: &mut Vec<Entry>, index: usize, category: &str) {
    if let Some(entry) = entries.get_mut(index) {
        entry.category = category.to_string();
    }
}


// ─── Tests ─────────────────────────────────────────────────────────────────
// `upsert_entry_replaces_on_same_path_and_pushes_new` is the Phase-0
// characterization test for `Registry::add_entry`, relocated here.

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
    fn upsert_entry_replaces_on_same_path_and_pushes_new() {
        let mut entries: Vec<Entry> = Vec::new();
        let mut e1 = entry("/a");
        e1.category = "old".into();
        let mut e2 = entry("/a");
        e2.category = "new".into();
        upsert_entry(&mut entries, e1);
        upsert_entry(&mut entries, e2);
        upsert_entry(&mut entries, entry("/b"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].category, "new");
        assert_eq!(entries[1].path, "/b");
    }

    #[test]
    fn remove_entry_returns_the_removed_entry() {
        let mut entries = vec![entry("/a"), entry("/b")];
        let removed = remove_entry(&mut entries, 0);
        assert_eq!(removed.map(|e| e.path), Some("/a".to_string()));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "/b");
    }

    #[test]
    fn tag_entry_adds_once_and_is_idempotent() {
        let mut entries = vec![entry("/a")];
        tag_entry(&mut entries, 0, "prod");
        tag_entry(&mut entries, 0, "prod");
        tag_entry(&mut entries, 0, "bastion");
        assert_eq!(entries[0].tags, vec!["prod".to_string(), "bastion".to_string()]);
        // Out-of-range index is a no-op.
        tag_entry(&mut entries, 5, "x");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn set_category_overwrites() {
        let mut entries = vec![entry("/a")];
        set_category(&mut entries, 0, "ssh-keys");
        set_category(&mut entries, 0, "certs");
        assert_eq!(entries[0].category, "certs");
    }
}
