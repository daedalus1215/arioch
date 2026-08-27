use crate::config::Config;
use crate::registry::{Entry, Registry};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;

const TICK_RATE: Duration = Duration::from_millis(100);

pub enum Event {
    Tick,
    Key(KeyEvent),
}

#[derive(PartialEq)]
pub enum Mode {
    View,
    Map,
    Suggestions,
    Search,
    Dialog,
    History,
    Investigate,
}

#[derive(Debug)]
pub struct DialogState {
    pub path: String,
    pub category: String,
    pub tags: String,
    pub description: String,
    pub alias: String,
    pub related: String,
    pub current_field: usize,
    pub is_new: bool,
}
pub struct App {
    pub config: Config,
    pub registry: Registry,
    pub selected_entry: Option<usize>,
    pub selected_category: Option<String>,
    pub mode: Mode,
    pub sidebar_expanded: bool,
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub scroll_offset: usize,
    pub message: Option<String>,
    pub show_help: bool,
    pub file_content: Option<String>,
    pub file_error: Option<String>,
    pub dialog: Option<DialogState>,
    pub suggestion_selected: usize,
    pub suggestion_scroll: usize,
    pub suggestion_filter: String,
    pub suggestion_accepted: usize,
    pub multi_selected: Vec<usize>,
    pub bulk_prompt: Option<String>,
    pub bulk_input: String,
    pub last_mtime: Option<std::time::SystemTime>,
    pub map_zoom: usize,
    pub map_scroll: usize,
    pub map_selected: Option<usize>,
    pub investigate_keys: Vec<crate::knowledge::DetectedKey>,
    pub investigate_selected: usize,
    pub investigate_scroll: usize,
    pub quit: bool,
}

impl App {
    pub fn new() -> Self {
        let config = Config::load();
        let registry = Registry::new();
        let selected = if registry.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        Self {
            config,
            registry,
            selected_entry: selected,
            selected_category: None,
            mode: Mode::View,
            sidebar_expanded: true,
            search_query: String::new(),
            search_results: Vec::new(),
            scroll_offset: 0,
            message: None,
            show_help: false,
            quit: false,
            file_content: None,
            file_error: None,
            dialog: None,
            suggestion_selected: 0,
            suggestion_scroll: 0,
            suggestion_filter: String::new(),
            suggestion_accepted: 0,
            multi_selected: Vec::new(),
            bulk_prompt: None,
            bulk_input: String::new(),
            last_mtime: None,
            map_zoom: 0,
            map_scroll: 0,
            map_selected: None,
            investigate_keys: Vec::new(),
            investigate_selected: 0,
            investigate_scroll: 0,
        }
    }

    pub fn tick(&mut self) {
        if self.mode != Mode::View {
            return;
        }
        if self.config.refresh_interval == 0 {
            return;
        }
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = expand_path(&entry.path);
                let entry_path = entry.path.clone();
                match std::fs::metadata(&path) {
                    Ok(metadata) => {
                        if let Ok(mtime) = metadata.modified() {
                            if let Some(last) = self.last_mtime {
                                if mtime != last {
                                    self.refresh_content();
                                    self.last_mtime = Some(mtime);
                                    self.set_message(&format!(
                                        "File updated: {}",
                                        entry_path
                                    ));
                                    return;
                                }
                            }
                            self.last_mtime = Some(mtime);
                        }
                    }
                    Err(_) => {
                        if self.file_content.is_some() {
                            self.file_content = None;
                            self.file_error = Some(format!(
                                "File no longer exists: {}",
                                entry_path
                            ));
                        }
                    }
                }
            }
        }
    }

    fn log_action(&self, action: &str, path: &str, details: &str) {
        let log_path = {
            let mut p = Config::config_dir();
            p.push("arioch");
            p.push("history.log");
            p
        };
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                format!(
                    "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    1970 + secs / 86400,
                    ((secs % 86400) / 3600) + 1,
                    ((secs % 3600) / 60) + 1,
                    (secs % 86400) / 3600,
                    (secs % 3600) / 60,
                    secs % 60
                )
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let line = format!("{} {} {} {}\n", timestamp, action, path, details);
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    pub fn read_history(&self) -> Vec<String> {
        let log_path = {
            let mut p = Config::config_dir();
            p.push("arioch");
            p.push("history.log");
            p
        };
        match std::fs::read_to_string(&log_path) {
            Ok(content) => content
                .lines()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| s.to_string())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.mode == Mode::Dialog {
            self.handle_dialog(key);
            return;
        }

        match key.code {
            crossterm::event::KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                return;
            }
            crossterm::event::KeyCode::Esc if self.show_help => {
                self.show_help = false;
                return;
            }
            crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
                if self.mode == Mode::Search {
                    self.mode = Mode::View;
                    self.search_query.clear();
                } else if self.mode == Mode::History {
                    self.mode = Mode::View;
                } else if self.mode == Mode::Map {
                    self.mode = Mode::View;
                } else if self.mode == Mode::Investigate {
                    self.mode = Mode::View;
                } else {
                    self.quit = true;
                }
            }
            _ => {
                match self.mode {
                    Mode::Search => self.handle_search(key),
                    Mode::Suggestions => self.handle_suggestions(key),
                    Mode::Map => self.handle_map(key),
                    Mode::Investigate => self.handle_investigate(key),
                    _ => self.handle_normal(key),
                }
            }
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        // Handle bulk prompt confirmation first
        if self.bulk_prompt.is_some() {
            let prompt = self.bulk_prompt.clone().unwrap_or_default();
            match key.code {
                crossterm::event::KeyCode::Char('y') if prompt.starts_with("Remove") => {
                    self.bulk_prompt = None;
                    self.bulk_input.clear();
                    self.bulk_remove();
                }
                crossterm::event::KeyCode::Enter => {
                    let input = self.bulk_input.trim().to_string();
                    self.bulk_prompt = None;
                    self.bulk_input.clear();
                    if input.is_empty() {
                        self.set_message("Empty input, cancelled");
                        return;
                    }
                    if prompt.starts_with("Add tag") {
                        self.bulk_tag(&input);
                    } else if prompt.starts_with("Set category") {
                        self.bulk_categorize(&input);
                    }
                }
                crossterm::event::KeyCode::Backspace => {
                    self.bulk_input.pop();
                }
                crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('n') => {
                    self.bulk_prompt = None;
                    self.bulk_input.clear();
                    self.set_message("Cancelled");
                }
                crossterm::event::KeyCode::Char(c) => {
                    self.bulk_input.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                self.select_next();
            }
            crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                self.select_prev();
            }
            crossterm::event::KeyCode::Char('h') | crossterm::event::KeyCode::Left => {
                self.sidebar_expanded = false;
            }
            crossterm::event::KeyCode::Char('l') | crossterm::event::KeyCode::Right => {
                self.sidebar_expanded = true;
            }
            crossterm::event::KeyCode::Char('m') => {
                self.mode = if matches!(self.mode, Mode::Map) {
                    Mode::View
                } else {
                    Mode::Map
                };
            }

            crossterm::event::KeyCode::Char('H') => {
                self.mode = Mode::History;
            }

            crossterm::event::KeyCode::Char('i') => {
                self.open_investigate();
            }
            crossterm::event::KeyCode::Char('s') => {
                self.mode = Mode::Suggestions;
                self.registry.scan_with_config(
                    &self.config.scan_paths,
                    &self.config.scan_patterns,
                    self.config.scan_depth,
                );
                self.set_message("Scanning for security files...");
            }
            crossterm::event::KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.search_query.clear();
                self.search_results.clear();
            }
            crossterm::event::KeyCode::Char('e') => {
                self.open_editor();
            }
            crossterm::event::KeyCode::Char('a') => {
                self.add_entry_dialog();
            }
            crossterm::event::KeyCode::Char('d') => {
                self.delete_selected();
            }
            crossterm::event::KeyCode::Char('c') => {
                self.change_category();
            }
            crossterm::event::KeyCode::Char('r') => {
                self.last_mtime = None;
                self.refresh_content();
            }
            crossterm::event::KeyCode::Char('J') => {
                self.scroll_down(5);
            }
            crossterm::event::KeyCode::Char('K') => {
                self.scroll_up(5);
            }
            crossterm::event::KeyCode::PageDown => {
                self.scroll_down(20);
            }
            crossterm::event::KeyCode::PageUp => {
                self.scroll_up(20);
            }
            // Multi-select
            crossterm::event::KeyCode::Char(' ') => {
                if let Some(idx) = self.selected_entry {
                    if let Some(pos) = self.multi_selected.iter().position(|&s| s == idx) {
                        self.multi_selected.remove(pos);
                    } else {
                        self.multi_selected.push(idx);
                    }
                }
            }
            crossterm::event::KeyCode::Esc => {
                if !self.multi_selected.is_empty() {
                    self.multi_selected.clear();
                    self.set_message("Selection cleared");
                }
            }
            crossterm::event::KeyCode::Char('t') if !self.multi_selected.is_empty() => {
                self.bulk_prompt = Some(format!(
                    "Add tag to {} entries: [type tag + Enter]",
                    self.multi_selected.len()
                ));
            }
            crossterm::event::KeyCode::Char('C') if !self.multi_selected.is_empty() => {
                self.bulk_prompt = Some(format!(
                    "Set category for {} entries: [type category + Enter]",
                    self.multi_selected.len()
                ));
            }
            crossterm::event::KeyCode::Char('D') if !self.multi_selected.is_empty() => {
                self.bulk_prompt = Some(format!(
                    "Remove {} entries? (y/n)",
                    self.multi_selected.len()
                ));
            }
            _ => {}
        }
    }

    fn handle_search(&mut self, key: KeyEvent) {
        match key.code {
            crossterm::event::KeyCode::Char(c) => {
                self.search_query.push(c);
                self.perform_search();
            }
            crossterm::event::KeyCode::Backspace => {
                self.search_query.pop();
                self.perform_search();
            }
            crossterm::event::KeyCode::Esc => {
                self.mode = Mode::View;
                self.search_query.clear();
                self.search_results.clear();
            }
            crossterm::event::KeyCode::Enter => {
                if !self.search_results.is_empty() {
                    self.selected_entry = Some(self.search_results[0]);
                    self.mode = Mode::View;
                }
            }
            _ => {}
        }
    }

    fn handle_suggestions(&mut self, key: KeyEvent) {
        let total = self.registry.suggestions.len();

        // Helper: get filtered suggestions
        let filtered: Vec<usize> = if self.suggestion_filter.is_empty() {
            (0..total).collect()
        } else {
            let filter = self.suggestion_filter.to_lowercase();
            (0..total)
                .filter(|&i| {
                    self.registry
                        .suggestions
                        .get(i)
                        .map(|p| p.to_string_lossy().to_lowercase().contains(&filter))
                        .unwrap_or(false)
                })
                .collect()
        };

        match key.code {
            crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                self.mode = Mode::View;
                self.suggestion_filter.clear();
                self.suggestion_selected = 0;
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                if !filtered.is_empty() {
                    let pos = filtered
                        .iter()
                        .position(|&f| f == self.suggestion_selected)
                        .unwrap_or(0);
                    let next = (pos + 1).min(filtered.len() - 1);
                    self.suggestion_selected = filtered[next];
                    if next >= self.suggestion_scroll + 12 {
                        self.suggestion_scroll = next - 11;
                    }
                }
            }
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                if !filtered.is_empty() {
                    let pos = filtered
                        .iter()
                        .position(|&f| f == self.suggestion_selected)
                        .unwrap_or(0);
                    let prev = if pos > 0 { pos - 1 } else { filtered.len() - 1 };
                    self.suggestion_selected = filtered[prev];
                    if prev < self.suggestion_scroll {
                        self.suggestion_scroll = prev;
                    }
                }
            }
            crossterm::event::KeyCode::Char('a') => {
                // Accept selected
                if self.registry.suggestions.get(self.suggestion_selected).is_some() {
                    let path = self.registry.suggestions[self.suggestion_selected]
                        .to_string_lossy()
                        .into_owned();
                    if !self.registry.entries.iter().any(|e| e.path == path) {
                        let category = guess_category(&path);
                        self.registry.add_entry(Entry {
                            path: path.clone(),
                            category,
                            tags: Vec::new(),
                            description: String::new(),
                            alias: None,
                            related: Vec::new(),
                        });
                        self.suggestion_accepted += 1;
                        if let Err(e) = self.registry.save() {
                            self.set_message(&format!("Save failed: {}", e));
                        }
                        self.set_message(&format!("Added: {}", path));
                    }
                    // Remove from suggestions
                    self.registry.suggestions.remove(self.suggestion_selected);
                    self.suggestion_selected = self.suggestion_selected
                        .min(self.registry.suggestions.len().saturating_sub(1));
                }
            }
            crossterm::event::KeyCode::Char('A') => {
                // Accept all
                let suggestions: Vec<String> = self
                    .registry
                    .suggestions
                    .iter()
                    .filter(|p| {
                        !self
                            .registry
                            .entries
                            .iter()
                            .any(|e| e.path == p.to_string_lossy())
                    })
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                for path in &suggestions {
                    let category = guess_category(path);
                    self.registry.add_entry(Entry {
                        path: path.clone(),
                        category,
                        tags: Vec::new(),
                        description: String::new(),
                        alias: None,
                        related: Vec::new(),
                    });
                }
                self.suggestion_accepted += suggestions.len();
                self.registry.suggestions.clear();
                if let Err(e) = self.registry.save() {
                    self.set_message(&format!("Save failed: {}", e));
                    return;
                }
                self.set_message(&format!("Added {} entries.", suggestions.len()));
                self.mode = Mode::View;
            }
            crossterm::event::KeyCode::Char('d') => {
                // Reject selected
                if self.registry.suggestions.get(self.suggestion_selected).is_some() {
                    self.registry.suggestions.remove(self.suggestion_selected);
                    self.suggestion_selected = self.suggestion_selected
                        .min(self.registry.suggestions.len().saturating_sub(1));
                }
            }
            crossterm::event::KeyCode::Char('e') | crossterm::event::KeyCode::Enter => {
                // Preview selected
                if let Some(path_buf) = self.registry.suggestions.get(self.suggestion_selected) {
                    let path = path_buf.to_string_lossy().into_owned();
                    let expanded = expand_path(&path);
                    match std::fs::read_to_string(&expanded) {
                        Ok(content) => {
                            self.file_content = Some(content);
                            self.file_error = None;
                            self.scroll_offset = 0;
                            self.set_message(&format!("Preview: {}", path));
                        }
                        Err(e) => {
                            self.set_message(&format!("Cannot read: {}", e));
                        }
                    }
                }
            }
            crossterm::event::KeyCode::Char('r') => {
                // Re-scan
                self.registry.scan_with_config(
                    &self.config.scan_paths,
                    &self.config.scan_patterns,
                    self.config.scan_depth,
                );
                self.suggestion_selected = 0;
                self.suggestion_filter.clear();
                self.set_message(&format!(
                    "Re-scanned: {} found",
                    self.registry.suggestions.len()
                ));
            }
            crossterm::event::KeyCode::Backspace => {
                self.suggestion_filter.pop();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.suggestion_filter.push(c);
            }
            _ => {}
        }
    }

    fn handle_map(&mut self, key: KeyEvent) {
        let total = self.registry.entries.len();

        match key.code {
            crossterm::event::KeyCode::Char('+') | crossterm::event::KeyCode::Char('=') => {
                self.map_zoom = (self.map_zoom + 1).min(1);
            }
            crossterm::event::KeyCode::Char('-') => {
                self.map_zoom = self.map_zoom.saturating_sub(1);
            }
            crossterm::event::KeyCode::Char('r') => {
                // Re-layout (reset scroll)
                self.map_scroll = 0;
                self.set_message("Map re-laid out");
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                if total > 1 {
                    let cur = self.map_selected.unwrap_or(0);
                    let next = (cur + 1).min(total - 1);
                    self.map_selected = Some(next);
                    // Auto-scroll if needed
                    if next >= self.map_scroll + 10 {
                        self.map_scroll = next - 9;
                    }
                }
            }
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                if total > 1 {
                    let cur = self.map_selected.unwrap_or(0);
                    let prev = if cur > 0 { cur - 1 } else { total - 1 };
                    self.map_selected = Some(prev);
                    if prev < self.map_scroll {
                        self.map_scroll = prev;
                    }
                }
            }
            crossterm::event::KeyCode::Enter => {
                if let Some(idx) = self.map_selected {
                    if self.registry.get_entry(idx).is_some() {
                        self.selected_entry = Some(idx);
                        self.mode = Mode::View;
                        self.refresh_content();
                    }
                }
            }
            crossterm::event::KeyCode::PageDown => {
                self.map_scroll = (self.map_scroll + 10).saturating_sub(1);
            }
            crossterm::event::KeyCode::PageUp => {
                self.map_scroll = self.map_scroll.saturating_sub(10);
            }
            _ => {}
        }
    }

    fn open_investigate(&mut self) {
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                if let Some(ref content) = self.file_content {
                    let kb = crate::knowledge::KnowledgeBase::load();
                    let file_type = match crate::syntax::detect_type(&entry.path, content.lines().next().unwrap_or("")) {
                        crate::syntax::FileType::Ssh => "ssh",
                        crate::syntax::FileType::Ini => "ini",
                        crate::syntax::FileType::Toml => "toml",
                        crate::syntax::FileType::Env => "env",
                        _ => "other",
                    };
                    self.investigate_keys = kb.detect(content, file_type);
                    self.investigate_selected = 0;
                    self.investigate_scroll = 0;
                    self.mode = Mode::Investigate;
                    self.set_message(&format!(
                        "Investigating {} ({} keys detected)",
                        entry.path,
                        self.investigate_keys.len()
                    ));
                } else {
                    self.set_message("No file content loaded. Press 'r' to refresh first.");
                }
            }
        } else {
            self.set_message("No entry selected.");
        }
    }

    fn handle_investigate(&mut self, key: KeyEvent) {
        let total = self.investigate_keys.len();

        match key.code {
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                if total > 1 {
                    let next = (self.investigate_selected + 1).min(total - 1);
                    self.investigate_selected = next;
                    if next >= self.investigate_scroll + 8 {
                        self.investigate_scroll = next - 7;
                    }
                }
            }
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                if total > 1 {
                    let prev = if self.investigate_selected > 0 {
                        self.investigate_selected - 1
                    } else {
                        total - 1
                    };
                    self.investigate_selected = prev;
                    if prev < self.investigate_scroll {
                        self.investigate_scroll = prev;
                    }
                }
            }
            crossterm::event::KeyCode::PageDown => {
                self.investigate_scroll = (self.investigate_scroll + 8).saturating_sub(1);
            }
            crossterm::event::KeyCode::PageUp => {
                self.investigate_scroll = self.investigate_scroll.saturating_sub(8);
            }
            _ => {}
        }
    }

    /// Get entry indices in visual display order (grouped by category, alphabetical).
    fn visual_order(&self) -> Vec<usize> {
        let categories = self.registry.categories();
        let mut order = Vec::new();
        for category in &categories {
            let indices = self.registry.entries_in_category(category);
            order.extend(indices);
        }
        order
    }

    fn select_next(&mut self) {
        let order = self.visual_order();
        if order.is_empty() {
            return;
        }
        if let Some(idx) = self.selected_entry {
            if let Some(pos) = order.iter().position(|&o| o == idx) {
                if pos + 1 < order.len() {
                    self.selected_entry = Some(order[pos + 1]);
                    self.refresh_content();
                }
            } else {
                self.selected_entry = Some(order[0]);
                self.refresh_content();
            }
        } else {
            self.selected_entry = Some(order[0]);
            self.refresh_content();
        }
    }

    fn select_prev(&mut self) {
        let order = self.visual_order();
        if order.is_empty() {
            return;
        }
        if let Some(idx) = self.selected_entry {
            if let Some(pos) = order.iter().position(|&o| o == idx) {
                let prev = if pos > 0 { pos - 1 } else { order.len() - 1 };
                self.selected_entry = Some(order[prev]);
                self.refresh_content();
            }
        }
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset += amount;
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn refresh_content(&mut self) {
        self.file_content = None;
        self.file_error = None;
        self.scroll_offset = 0;

        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = expand_path(&entry.path);

                // Check file size before loading
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if metadata.len() > self.config.max_file_size as u64 {
                        self.file_error = Some(format!(
                            "File too large ({} bytes, max {}): {}",
                            metadata.len(),
                            self.config.max_file_size,
                            path
                        ));
                        return;
                    }
                }

                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        self.file_content = Some(content);
                    }
                    Err(e) => {
                        self.file_error = Some(format!("Cannot read {}: {}", path, e));
                    }
                }
            }
        }
    }

    fn open_editor(&mut self) {
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = expand_path(&entry.path);
                let entry_path = entry.path.clone();
                let editor = self.config.editor();
                let mut cmd = std::process::Command::new(&editor);
                cmd.arg(&path);
                // Disable raw mode while editor runs, re-enable after
                let _ = crossterm::terminal::disable_raw_mode();
                let status = cmd.status();
                let _ = crossterm::terminal::enable_raw_mode();
                match status {
                    Ok(_) => {
                        self.refresh_content();
                        self.last_mtime = None;
                        self.set_message(&format!("Edited: {}", entry_path));
                    }
                    Err(e) => {
                        self.set_message(&format!("Editor failed: {}", e));
                    }
                }
            }
        }
    }

    fn add_entry_dialog(&mut self) {
        self.mode = Mode::Dialog;
        self.dialog = Some(DialogState {
            path: "~/.ssh/".to_string(),
            category: String::new(),
            tags: String::new(),
            description: String::new(),
            alias: String::new(),
            related: String::new(),
            current_field: 0,
            is_new: true,
        });
    }

    fn handle_dialog(&mut self, key: KeyEvent) {
        let dialog = match &mut self.dialog {
            Some(d) => d,
            None => {
                self.mode = Mode::View;
                return;
            }
        };

        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = Mode::View;
                self.dialog = None;
            }
            crossterm::event::KeyCode::Enter => {
                self.save_dialog();
            }
            crossterm::event::KeyCode::Tab => {
                dialog.current_field = (dialog.current_field + 1) % 6;
            }
            crossterm::event::KeyCode::Backspace => {
                self.backspace_dialog_field();
            }
            crossterm::event::KeyCode::Char('d') if !dialog.is_new => {
                self.delete_from_dialog();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.append_dialog_field(c);
            }
            crossterm::event::KeyCode::Up => {
                dialog.current_field = dialog.current_field.wrapping_sub(1);
            }
            crossterm::event::KeyCode::Down => {
                dialog.current_field = (dialog.current_field + 1) % 6;
            }
            _ => {}
        }
    }

    fn append_dialog_field(&mut self, c: char) {
        if let Some(dialog) = &mut self.dialog {
            let field = match dialog.current_field {
                0 => &mut dialog.path,
                1 => &mut dialog.category,
                2 => &mut dialog.tags,
                3 => &mut dialog.description,
                4 => &mut dialog.alias,
                5 => &mut dialog.related,
                _ => return,
            };
            field.push(c);


            // Auto-detect category when path changes and category is empty
            if dialog.current_field == 0 && dialog.category.is_empty() {
                let detected = guess_category(&dialog.path);
                if !detected.is_empty() {
                    dialog.category = detected;
                }
            }
        }
    }

    fn backspace_dialog_field(&mut self) {
        if let Some(dialog) = &mut self.dialog {
            let field = match dialog.current_field {
                0 => &mut dialog.path,
                1 => &mut dialog.category,
                2 => &mut dialog.tags,
                3 => &mut dialog.description,
                4 => &mut dialog.alias,
                5 => &mut dialog.related,
                _ => return,
            };
            field.pop();
        }
    }

    fn save_dialog(&mut self) {
        let dialog = match self.dialog.take() {
            Some(d) => d,
            None => {
                self.mode = Mode::View;
                return;
            }
        };

        if dialog.path.trim().is_empty() {
            self.mode = Mode::View;
            self.set_message("Path is required");
            return;
        }

        let tags: Vec<String> = dialog
            .tags
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let related: Vec<String> = dialog
            .related
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Check if path already exists
        let is_update = self
            .registry
            .entries
            .iter()
            .any(|e| e.path == dialog.path);

        let entry = Entry {
            path: dialog.path.trim().to_string(),
            category: dialog.category.trim().to_string(),
            tags,
            description: dialog.description.trim().to_string(),
            alias: if dialog.alias.trim().is_empty() {
                None
            } else {
                Some(dialog.alias.trim().to_string())
            },
            related,
        };

        self.registry.add_entry(entry);

        if let Err(e) = self.registry.save() {
            self.mode = Mode::View;
            self.set_message(&format!("Save failed: {}", e));
            return;
        }

        // Select the newly added/updated entry
        self.selected_entry = self
            .registry
            .entries
            .iter()
            .position(|e| e.path == dialog.path.trim())
            .or_else(|| self.registry.entries.first().map(|_| 0));

        self.mode = Mode::View;
        self.refresh_content();
        self.log_action(
            if is_update { "edit" } else { "add" },
            &dialog.path,
            &format!("category={}", dialog.category),
        );
        self.set_message(if is_update {
            "Entry updated"
        } else {
            "Entry added"
        });
    }

    fn delete_from_dialog(&mut self) {
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = entry.path.clone();
                self.registry.remove_entry(idx);
                if let Err(e) = self.registry.save() {
                    self.set_message(&format!("Save failed: {}", e));
                    return;
                }
                self.selected_entry = if self.registry.entries.is_empty() {
                    None
                } else if idx >= self.registry.entries.len() {
                    Some(idx - 1)
                } else {
                    Some(idx)
                };
                self.refresh_content();
                self.mode = Mode::View;
                self.dialog = None;
                self.set_message(&format!("Removed: {}", path));
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(idx) = self.selected_entry {
            let entry = self.registry.get_entry(idx).cloned();
            if let Some(e) = entry {
                self.registry.remove_entry(idx);
                if let Err(err) = self.registry.save() {
                    self.set_message(&format!("Save failed: {}", err));
                    return;
                }
                self.selected_entry = if self.registry.entries.is_empty() {
                    None
                } else if idx >= self.registry.entries.len() {
                    Some(idx - 1)
                } else {
                    Some(idx)
                };
                self.refresh_content();
                self.log_action("remove", &e.path, "user-requested");
                self.set_message(&format!("Removed: {}", e.path));
            }
        }
    }

    fn bulk_remove(&mut self) {
        let count = self.multi_selected.len();
        // Remove from highest index to lowest to avoid index shifting
        let mut indices: Vec<usize> = self.multi_selected.clone();
        indices.sort_by(|a, b| b.cmp(a));
        for idx in indices {
            self.registry.remove_entry(idx);
        }
        if let Err(e) = self.registry.save() {
            self.set_message(&format!("Save failed: {}", e));
            return;
        }
        self.multi_selected.clear();
        self.selected_entry = if self.registry.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.refresh_content();
        self.log_action("remove", "*", &format!("bulk-remove count={}", count));
        self.set_message(&format!("Removed {} entries", count));
    }

    fn bulk_tag(&mut self, tag: &str) {
        let count = self.multi_selected.len();
        for &idx in &self.multi_selected {
            if let Some(entry) = self.registry.get_entry_mut(idx) {
                if !entry.tags.iter().any(|t| t == tag) {
                    entry.tags.push(tag.to_string());
                }
            }
        }
        if let Err(e) = self.registry.save() {
            self.set_message(&format!("Save failed: {}", e));
            return;
        }
        self.multi_selected.clear();
        self.log_action("edit", "*", &format!("bulk-tag tag={} count={}", tag, count));
        self.set_message(&format!("Tagged {} entries with '{}'", count, tag));
    }

    fn bulk_categorize(&mut self, category: &str) {
        let count = self.multi_selected.len();
        for &idx in &self.multi_selected {
            if let Some(entry) = self.registry.get_entry_mut(idx) {
                entry.category = category.to_string();
            }
        }
        if let Err(e) = self.registry.save() {
            self.set_message(&format!("Save failed: {}", e));
            return;
        }
        self.multi_selected.clear();
        self.log_action("edit", "*", &format!("bulk-categorize category={} count={}", category, count));
        self.set_message(&format!("Categorized {} entries as '{}'", count, category));
    }

    fn change_category(&mut self) {
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                self.mode = Mode::Dialog;
                self.dialog = Some(DialogState {
                    path: entry.path.clone(),
                    category: entry.category.clone(),
                    tags: entry.tags.join(", "),
                    description: entry.description.clone(),
                    alias: entry.alias.clone().unwrap_or_default(),
                    related: entry.related.join(", "),
                    current_field: 0,
                    is_new: false,
                });
                return;
            }
        }
        self.set_message("No entry selected to edit");
    }


    fn perform_search(&mut self) {
        self.search_results.clear();
        if self.search_query.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        for (i, entry) in self.registry.entries.iter().enumerate() {
            if entry.path.to_lowercase().contains(&query)
                || entry.category.to_lowercase().contains(&query)
                || entry.description.to_lowercase().contains(&query)
                || entry.tags.iter().any(|t| t.to_lowercase().contains(&query))
                || entry
                    .alias
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(&query))
                    .unwrap_or(false)
            {
                self.search_results.push(i);
            }
        }
    }

    fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    pub fn next_event() -> Event {
        if event::poll(TICK_RATE).unwrap_or(false) {
            match event::read().unwrap() {
                CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                    return Event::Key(key);
                }
                _ => {}
            }
        }
        Event::Tick
    }
}

fn guess_category(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains(".ssh") || lower.contains("ssh_key") || lower.contains("id_") {
        return "ssh-keys".into();
    }
    if lower.contains(".pem") || lower.contains(".crt") || lower.contains(".cert") {
        return "certificates".into();
    }
    if lower.contains(".gnupg") || lower.contains("gpg") {
        return "gpg".into();
    }
    if lower.contains("credentials") || lower.contains("secret") || lower.contains(".env") {
        return "secrets".into();
    }
    if lower.contains("/etc/") {
        return "system-config".into();
    }
    if lower.contains(".config") {
        return "app-config".into();
    }
    "other".into()
}

fn expand_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}
