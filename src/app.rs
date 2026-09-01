use crate::config::Config;
use crate::registry::{Entry, Registry};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;
use std::path::Path;
use crate::domain::ports::{
    AnnotationStore, AuditLog, Clipboard, Editor, Filesystem, RegistryStore,
};

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
    Edit,
    Annotate,
    AnnotView,
}

/// Inline editor buffer for the currently selected file.
#[derive(Debug, Clone)]
pub struct EditState {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub inserting: bool,
    pub dirty: bool,
    pub trailing_newline: bool,
    pub prompting_save: bool,
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
    pub fs: Box<dyn Filesystem>,
    pub editor: Box<dyn Editor>,
    pub clipboard: Box<dyn Clipboard>,
    pub audit: Box<dyn AuditLog>,
    pub registry_store: Box<dyn RegistryStore>,
    pub annotation_store: Box<dyn AnnotationStore>,
    pub selected_entry: Option<usize>,
    pub selected_category: Option<String>,
    pub mode: Mode,
    pub sidebar_expanded: bool,
    pub sidebar_width: usize,
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub scroll_offset: usize,
    pub message: Option<String>,
    pub show_help: bool,
    pub file_content: Option<String>,
    pub baseline_content: Option<String>,
    pub baseline_entry: Option<usize>,
    pub show_diff: bool,
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
    pub last_index_mtime: Option<std::time::SystemTime>,
    pub map_zoom: usize,
    pub map_scroll: usize,
    pub map_selected: Option<usize>,
    pub investigate_keys: Vec<crate::knowledge::DetectedKey>,
    pub investigate_selected: usize,
    pub investigate_scroll: usize,
    pub quit: bool,
    pub edit: Option<EditState>,
    pub view_line: usize,
    pub annot_anchor: usize,
    pub annot_cursor: usize,
    pub annot_text: Option<String>,
    pub annot_view: Option<usize>,
    pub annotations: Vec<crate::annotations::Annotation>,
}


/// I/O ports injected at the composition root (`run_tui`).
pub struct AppPorts {
    pub fs: Box<dyn Filesystem>,
    pub editor: Box<dyn Editor>,
    pub clipboard: Box<dyn Clipboard>,
    pub audit: Box<dyn AuditLog>,
    pub registry_store: Box<dyn RegistryStore>,
    pub annotation_store: Box<dyn AnnotationStore>,
}
impl App {
    pub fn new(config: Config, registry: Registry, ports: AppPorts) -> Self {
        let selected = if registry.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        let annotations = ports.annotation_store.load();
        Self {
            config,
            registry,
            fs: ports.fs,
            editor: ports.editor,
            clipboard: ports.clipboard,
            audit: ports.audit,
            registry_store: ports.registry_store,
            annotation_store: ports.annotation_store,
            selected_entry: selected,
            selected_category: None,
            mode: Mode::View,
            sidebar_expanded: true,
            sidebar_width: 35,
            search_query: String::new(),
            search_results: Vec::new(),
            scroll_offset: 0,
            message: None,
            show_help: false,
            file_content: None,
            baseline_content: None,
            baseline_entry: None,
            show_diff: false,
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
            last_index_mtime: None,
            map_zoom: 0,
            map_scroll: 0,
            map_selected: None,
            investigate_keys: Vec::new(),
            investigate_selected: 0,
            investigate_scroll: 0,
            quit: false,
            edit: None,
            view_line: 0,
            annot_anchor: 0,
            annot_cursor: 0,
            annot_text: None,
            annot_view: None,
            annotations,
        }
    }

    pub fn tick(&mut self) {
        if self.mode != Mode::View {
            return;
        }
        if self.config.refresh_interval == 0 {
            return;
        }
        // Watch index file for external changes
        let index_path = {
            let mut p = Config::config_dir();
            p.push("arioch");
            p.push("index.toml");
            p
        };
        if let Ok(meta) = self.fs.metadata(&index_path) {
            let mtime = meta.modified;
            if let Some(last) = self.last_index_mtime {
                if mtime != last {
                    // Reload registry
                    match Registry::load() {
                        Ok(new_registry) => {
                            let selected_path = self
                                .selected_entry
                                .and_then(|i| self.registry.get_entry(i))
                                .map(|e| e.path.clone());
                            let entry_count = new_registry.entries.len();
                            self.registry = new_registry;
                            // Re-select by path if still exists
                            if let Some(ref path) = selected_path {
                                self.selected_entry = self
                                    .registry
                                    .entries
                                    .iter()
                                    .position(|e| &e.path == path);
                            }
                            self.refresh_content();
                            self.set_message(&format!(
                                "Index reloaded ({} entries)",
                                entry_count
                            ));
                            self.last_index_mtime = Some(mtime);
                            return;
                        }
                        Err(e) => {
                            self.set_message(&format!("Index reload failed: {}", e));
                            self.last_index_mtime = Some(mtime);
                            return;
                        }
                    }
                }
            }
            self.last_index_mtime = Some(mtime);
        }
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = crate::domain::rules::expand_path(&entry.path);
                let entry_path = entry.path.clone();
                match self.fs.metadata(Path::new(&path)) {
                    Ok(metadata) => {
                        let mtime = metadata.modified;
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
        let _ = self.audit.append(action, path, details);
    }

    pub fn read_history(&self) -> Vec<String> {
        self.audit.recent(20)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.mode == Mode::Dialog {
            self.handle_dialog(key);
            return;
        }

        match key.code {
            // In edit insert mode, every key is text except Esc
            _ if self.edit_inserting()
                && !matches!(key.code, crossterm::event::KeyCode::Esc) =>
            {
                self.handle_edit(key);
                return;
            }
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
                } else if self.mode == Mode::Edit {
                    if self.edit.as_ref().map(|e| e.prompting_save).unwrap_or(false) {
                        if let Some(e) = &mut self.edit {
                            e.prompting_save = false;
                        }
                    } else if self.edit_inserting() {
                        if let Some(e) = &mut self.edit {
                            e.inserting = false;
                        }
                    } else if self.edit.as_ref().map(|e| e.dirty).unwrap_or(false) {
                        if let Some(e) = &mut self.edit {
                            e.prompting_save = true;
                        }
                        self.set_message("Save changes? y:save  n:discard  Esc:keep editing");
                    } else {
                        self.edit = None;
                        self.mode = Mode::View;
                    }
                } else if self.mode == Mode::Annotate {
                    self.annot_escape();
                } else if self.mode == Mode::AnnotView {
                    self.annot_view = None;
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
                    Mode::Edit => self.handle_edit(key),
                    Mode::Annotate => self.handle_annotate(key),
                    Mode::AnnotView => self.handle_annot_view(key),
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
            crossterm::event::KeyCode::Char('<') => {
                self.sidebar_width = (self.sidebar_width.saturating_sub(2)).max(10);
            }
            crossterm::event::KeyCode::Char('>') => {
                self.sidebar_width = (self.sidebar_width + 2).min(80);
            }
            crossterm::event::KeyCode::Char('1')
            | crossterm::event::KeyCode::Char('2')
            | crossterm::event::KeyCode::Char('3')
            | crossterm::event::KeyCode::Char('4')
            | crossterm::event::KeyCode::Char('5')
            | crossterm::event::KeyCode::Char('6')
            | crossterm::event::KeyCode::Char('7')
            | crossterm::event::KeyCode::Char('8')
            | crossterm::event::KeyCode::Char('9') => {
                self.quick_jump(key.code);
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
            crossterm::event::KeyCode::Char('c') => {
                self.copy_path();
            }
            crossterm::event::KeyCode::Char('s') => {
                self.mode = Mode::Suggestions;
                self.registry.scan_with_config(
                    &self.config.scan_paths,
                    &self.config.exclude_paths,
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
            crossterm::event::KeyCode::Char('E') => {
                self.enter_edit();
            }
            crossterm::event::KeyCode::Char('v') => {
                self.start_annotate();
            }
            crossterm::event::KeyCode::Char('g') => {
                self.next_annotation();
            }
            crossterm::event::KeyCode::Char('A') => {
                self.view_annotation();
            }
            crossterm::event::KeyCode::Char('+') | crossterm::event::KeyCode::Char('=') => {
                self.move_view_line(1);
            }
            crossterm::event::KeyCode::Char('-') => {
                self.move_view_line(-1);
            }
            crossterm::event::KeyCode::Char('d') => {
                self.show_diff = !self.show_diff;
            }
            crossterm::event::KeyCode::Char('a') => {
                self.add_entry_dialog();
            }
            crossterm::event::KeyCode::Char('x') => {
                self.delete_selected();
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
            crossterm::event::KeyCode::Char('C') => {
                if self.multi_selected.is_empty() {
                    self.change_category();
                } else {
                    self.bulk_prompt = Some(format!(
                        "Set category for {} entries: [type category + Enter]",
                        self.multi_selected.len()
                    ));
                }
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
                        let category = crate::domain::rules::guess_category_tui(&path);
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
                    let category = crate::domain::rules::guess_category_tui(path);
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
                    let expanded = crate::domain::rules::expand_path(&path);
                    match self.fs.read_to_string(Path::new(&expanded)) {
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
                    &self.config.exclude_paths,
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
    fn edit_inserting(&self) -> bool {
        self.edit.as_ref().map(|e| e.inserting).unwrap_or(false)
    }


    // ── Inline editing ────────────────────────────────────────────────────

    fn enter_edit(&mut self) {
        let idx = match self.selected_entry {
            Some(i) => i,
            None => {
                self.set_message("No entry selected");
                return;
            }
        };
        let entry = match self.registry.get_entry(idx) {
            Some(e) => e.clone(),
            None => return,
        };
        let content = match &self.file_content {
            Some(c) => c.clone(),
            None => {
                self.set_message("Cannot edit: no content loaded — press 'r' to refresh");
                return;
            }
        };
        let path = crate::domain::rules::expand_path(&entry.path);
        #[cfg(unix)]
        if let Ok(meta) = self.fs.metadata(Path::new(&path)) {
            if meta.mode & 0o200 == 0 {
                self.set_message(&format!(
                    "File is read-only (mode {:o}) — use external editor (e)",
                    meta.mode & 0o777
                ));
                return;
            }
        }

        let trailing_newline = !content.is_empty() && content.ends_with('\n');
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        self.mode = Mode::Edit;
        self.edit = Some(EditState {
            lines,
            cursor_line: 0,
            cursor_col: 0,
            inserting: false,
            dirty: false,
            trailing_newline,
            prompting_save: false,
        });
        self.set_message("Edit mode — s:save  Esc:quit");
    }

    fn handle_edit(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;

        // Save-prompt state takes precedence
        if self.edit.as_ref().map(|e| e.prompting_save).unwrap_or(false) {
            match key.code {
                KeyCode::Char('y') => self.save_edit(),
                KeyCode::Char('n') => self.discard_edit(),
                _ => {}
            }
            return;
        }

        // Keys that transition out of edit mode
        match key.code {
            KeyCode::Char('s') => {
                self.save_edit();
                return;
            }
            _ => {}
        }

        let edit = match self.edit.as_mut() {
            Some(e) => e,
            None => {
                self.mode = Mode::View;
                return;
            }
        };

        let line_len = edit
            .lines
            .get(edit.cursor_line)
            .map(|l| l.chars().count())
            .unwrap_or(0);

        // Insert mode: all characters are text
        if edit.inserting {
            match key.code {
                KeyCode::Char(c) => edit_insert_char(edit, c),
                KeyCode::Backspace => edit_backspace(edit),
                KeyCode::Enter => edit_enter(edit),
                _ => {}
            }
            return;
        }

        // Normal-mode commands
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if edit.cursor_line + 1 < edit.lines.len() {
                    edit.cursor_line += 1;
                }
                edit.cursor_col = edit
                    .cursor_col
                    .min(edit.lines[edit.cursor_line].chars().count());
            }
            KeyCode::Up | KeyCode::Char('k') => {
                edit.cursor_line = edit.cursor_line.saturating_sub(1);
                edit.cursor_col = edit
                    .cursor_col
                    .min(edit.lines[edit.cursor_line].chars().count());
            }
            KeyCode::Right | KeyCode::Char('l') => {
                edit.cursor_col = (edit.cursor_col + 1).min(line_len);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                edit.cursor_col = edit.cursor_col.saturating_sub(1);
            }
            KeyCode::Char('i') => {
                edit.inserting = true;
            }
            KeyCode::Char('a') => {
                if edit.cursor_col < line_len {
                    edit.cursor_col += 1;
                } else if edit.cursor_line + 1 < edit.lines.len() {
                    edit.cursor_line += 1;
                    edit.cursor_col = 0;
                }
                edit.inserting = true;
            }
            KeyCode::Char('A') => {
                edit.cursor_col = line_len;
                edit.inserting = true;
            }
            KeyCode::Char('0') => {
                edit.cursor_col = 0;
            }
            KeyCode::Char('$') => {
                edit.cursor_col = line_len;
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                edit_delete_char(edit);
            }
            KeyCode::Char('o') => {
                edit.lines.insert(edit.cursor_line + 1, String::new());
                edit.cursor_line += 1;
                edit.cursor_col = 0;
                edit.inserting = true;
                edit.dirty = true;
            }
            KeyCode::Char('O') => {
                edit.lines.insert(edit.cursor_line, String::new());
                edit.cursor_col = 0;
                edit.inserting = true;
                edit.dirty = true;
            }
            KeyCode::PageDown => {
                edit.cursor_line = (edit.cursor_line + 20)
                    .min(edit.lines.len().saturating_sub(1));
                edit.cursor_col = edit
                    .cursor_col
                    .min(edit.lines[edit.cursor_line].chars().count());
            }
            KeyCode::PageUp => {
                edit.cursor_line = edit.cursor_line.saturating_sub(20);
                edit.cursor_col = edit
                    .cursor_col
                    .min(edit.lines[edit.cursor_line].chars().count());
            }
            _ => {}
        }
    }

    fn save_edit(&mut self) {
        let snapshot = match self.edit.clone() {
            Some(e) => e,
            None => {
                self.mode = Mode::View;
                return;
            }
        };
        let idx = match self.selected_entry {
            Some(i) => i,
            None => {
                self.mode = Mode::View;
                return;
            }
        };
        let entry = match self.registry.get_entry(idx) {
            Some(e) => e.clone(),
            None => {
                self.mode = Mode::View;
                return;
            }
        };

        let mut content = snapshot.lines.join("\n");
        if snapshot.trailing_newline {
            content.push('\n');
        }

        let path = crate::domain::rules::expand_path(&entry.path);
        if let Ok(original) = self.fs.read_to_string(Path::new(&path)) {
            if original == content {
                self.file_content = Some(content);
                self.baseline_content = Some(self.file_content.clone().unwrap_or_default());
                self.baseline_entry = Some(idx);
                self.edit = None;
                self.mode = Mode::View;
                self.set_message("No changes");
                return;
            }
        }

        match self.fs.write(Path::new(&path), &content) {
            Ok(()) => {
                self.file_content = Some(content.clone());
                self.baseline_content = Some(content.clone());
                self.baseline_entry = Some(idx);
                self.last_mtime = None;
                self.edit = None;
                self.mode = Mode::View;
                self.log_action("edit", &entry.path, "inline");
                self.set_message(&format!("Saved: {}", entry.path));
            }
            Err(e) => {
                self.edit = Some(snapshot);
                self.set_message(&format!("Save failed: {}", e));
            }
        }
    }

    fn discard_edit(&mut self) {
        self.edit = None;
        self.mode = Mode::View;
        self.refresh_content();
        self.set_message("Changes discarded");
    }

    // ── Inline annotations ────────────────────────────────────────────────

    fn start_annotate(&mut self) {
        if self.file_content.is_none() {
            self.set_message("No content to annotate — press 'r' to refresh");
            return;
        }
        self.mode = Mode::Annotate;
        self.annot_anchor = self.view_line;
        self.annot_cursor = self.view_line;
        self.annot_text = None;
    }

    fn handle_annotate(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;

        // Comment popover open: capture text
        if self.annot_text.is_some() {
            match key.code {
                KeyCode::Char(c) => {
                    if let Some(t) = self.annot_text.as_mut() {
                        t.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(t) = self.annot_text.as_mut() {
                        t.pop();
                    }
                }
                KeyCode::Enter => self.save_annotation(),
                _ => {}
            }
            return;
        }

        let total = self
            .file_content
            .as_ref()
            .map(|c| c.lines().count().max(1))
            .unwrap_or(1);

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.annot_cursor = (self.annot_cursor + 1).min(total - 1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.annot_cursor = self.annot_cursor.saturating_sub(1);
            }
            KeyCode::Char('v') => {
                self.annot_anchor = self.annot_cursor;
            }
            KeyCode::Char('c') => {
                self.annot_text = Some(String::new());
            }
            KeyCode::Char('g') => {
                self.next_annotation();
            }
            KeyCode::Char('A') => {
                self.view_annotation();
            }
            _ => {}
        }
    }

    fn save_annotation(&mut self) {
        let text = match self.annot_text.as_mut() {
            Some(t) => t.trim().to_string(),
            None => return,
        };
        if text.is_empty() {
            self.set_message("Comment text is required");
            return;
        }
        let idx = match self.selected_entry {
            Some(i) => i,
            None => {
                self.mode = Mode::View;
                return;
            }
        };
        let entry = match self.registry.get_entry(idx) {
            Some(e) => e.clone(),
            None => {
                self.mode = Mode::View;
                return;
            }
        };
        let start = self.annot_anchor.min(self.annot_cursor) + 1;
        let end = self.annot_anchor.max(self.annot_cursor) + 1;
        self.annotations.push(crate::annotations::Annotation {
            path: entry.path.clone(),
            start,
            end,
            text,
            created: crate::domain::rules::iso_now(),
        });
        match crate::annotations::AnnotationsFile::save(&self.annotations) {
            Ok(()) => {
                self.mode = Mode::View;
                self.annot_text = None;
                self.log_action("annotate", &entry.path, &format!("lines={}-{}", start, end));
                self.set_message(&format!("Annotated lines {}-{}", start, end));
            }
            Err(e) => {
                self.annotations.pop();
                self.set_message(&format!("Save failed: {}", e));
            }
        }
    }

    fn annot_escape(&mut self) {
        if self.annot_text.is_some() {
            self.annot_text = None;
        } else {
            self.mode = Mode::View;
            self.set_message("Selection cancelled");
        }
    }

    fn next_annotation(&mut self) {
        let idx = match self.selected_entry {
            Some(i) => i,
            None => return,
        };
        let entry = match self.registry.get_entry(idx) {
            Some(e) => e.clone(),
            None => return,
        };
        let mut idxs: Vec<usize> = (0..self.annotations.len())
            .filter(|&i| self.annotations[i].path == entry.path)
            .collect();
        if idxs.is_empty() {
            self.set_message("No annotations for this file");
            return;
        }
        idxs.sort_by_key(|&i| self.annotations[i].start);
        let current_line = self.view_line + 1;
        let next = idxs
            .iter()
            .copied()
            .find(|&i| self.annotations[i].start > current_line)
            .or_else(|| idxs.first().copied());
        if let Some(i) = next {
            self.view_line = self.annotations[i].start.saturating_sub(1);
            self.scroll_offset = self.view_line;
            self.annot_view = Some(i);
            self.mode = Mode::AnnotView;
        }
    }

    fn view_annotation(&mut self) {
        let idx = match self.selected_entry {
            Some(i) => i,
            None => return,
        };
        let entry = match self.registry.get_entry(idx) {
            Some(e) => e.clone(),
            None => return,
        };
        let line = self.view_line + 1;
        match (0..self.annotations.len()).find(|&i| {
            self.annotations[i].path == entry.path && self.annotations[i].covers(line)
        }) {
            Some(i) => {
                self.annot_view = Some(i);
                self.mode = Mode::AnnotView;
            }
            None => {
                self.set_message(&format!("No annotation at line {}", line));
            }
        }
    }

    fn handle_annot_view(&mut self, key: KeyEvent) {
        if key.code == crossterm::event::KeyCode::Char('d') {
            if let Some(i) = self.annot_view {
                let ann = self.annotations[i].clone();
                self.annotations.remove(i);
                match crate::annotations::AnnotationsFile::save(&self.annotations) {
                    Ok(()) => {
                        self.log_action(
                            "unannotate",
                            &ann.path,
                            &format!("lines={}-{}", ann.start, ann.end),
                        );
                        self.set_message(&format!(
                            "Annotation removed (lines {}-{})",
                            ann.start, ann.end
                        ));
                    }
                    Err(e) => {
                        self.annotations.insert(i, ann);
                        self.set_message(&format!("Save failed: {}", e));
                    }
                }
            }
            self.annot_view = None;
            self.mode = Mode::View;
        }
    }

    fn move_view_line(&mut self, delta: isize) {
        let total = self
            .file_content
            .as_ref()
            .map(|c| c.lines().count().max(1))
            .unwrap_or(1);
        let cur = self.view_line as isize;
        let next = (cur + delta).clamp(0, (total - 1) as isize);
        self.view_line = next as usize;
    }

    fn visual_order(&self) -> Vec<usize> {
        crate::domain::rules::visual_order(&self.registry.entries)
    }

    fn quick_jump(&mut self, code: crossterm::event::KeyCode) {
        if let crossterm::event::KeyCode::Char(c) = code {
            let n = c.to_digit(10).unwrap_or(0) as usize - 1;
            let order = self.visual_order();
            if n < order.len() {
                self.selected_entry = Some(order[n]);
                self.refresh_content();
            }
        }
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
        self.view_line = 0;

        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = crate::domain::rules::expand_path(&entry.path);

                // Check file size before loading
                if let Ok(metadata) = self.fs.metadata(Path::new(&path)) {
                    if metadata.len > self.config.max_file_size as u64 {
                        self.file_error = Some(format!(
                            "File too large ({} bytes, max {}): {}",
                            metadata.len,
                            self.config.max_file_size,
                            path
                        ));
                        return;
                    }
                }

                match self.fs.read_to_string(Path::new(&path)) {
                    Ok(content) => {
                        // Set baseline on first load of this entry
                        if self.baseline_entry != Some(idx) {
                            self.baseline_content = Some(content.clone());
                            self.baseline_entry = Some(idx);
                        }
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
                let path = crate::domain::rules::expand_path(&entry.path);
                let entry_path = entry.path.clone();
                // Disable raw mode while editor runs, re-enable after
                let _ = crossterm::terminal::disable_raw_mode();
                let result = self.editor.launch(Path::new(&path));
                let _ = crossterm::terminal::enable_raw_mode();
                match result {
                    Ok(()) => {
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

    fn copy_path(&mut self) {
        if let Some(idx) = self.selected_entry {
            if let Some(entry) = self.registry.get_entry(idx) {
                let path = crate::domain::rules::expand_path(&entry.path);
                let copied = self.copy_to_clipboard(&path);
                if copied {
                    self.set_message(&format!("Copied: {}", path));
                } else {
                    self.set_message("No clipboard tool found (install xclip or wl-clipboard)");
                }
            }
        }
    }

    fn copy_to_clipboard(&self, text: &str) -> bool {
        self.clipboard.copy(text)
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
                let detected = crate::domain::rules::guess_category_tui(&dialog.path);
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


/// Insert a character at the cursor position.
fn edit_insert_char(edit: &mut EditState, c: char) {
    let line = edit.lines[edit.cursor_line].clone();
    let mut chars: Vec<char> = line.chars().collect();
    chars.insert(edit.cursor_col, c);
    edit.lines[edit.cursor_line] = chars.into_iter().collect();
    edit.cursor_col += 1;
    edit.dirty = true;
}
/// Delete the char under the cursor (or merge with the next line at end-of-line).
fn edit_delete_char(edit: &mut EditState) {
    let line = edit.lines[edit.cursor_line].clone();
    let mut chars: Vec<char> = line.chars().collect();
    if edit.cursor_col < chars.len() {
        chars.remove(edit.cursor_col);
        edit.lines[edit.cursor_line] = chars.into_iter().collect();
    } else if edit.cursor_line + 1 < edit.lines.len() {
        let next = edit.lines[edit.cursor_line + 1].clone();
        edit.lines[edit.cursor_line].push_str(&next);
        edit.lines.remove(edit.cursor_line + 1);
    }
    edit.dirty = true;
}

/// Delete the char before the cursor (or merge with the previous line at start-of-line).
fn edit_backspace(edit: &mut EditState) {
    if edit.cursor_col > 0 {
        let line = edit.lines[edit.cursor_line].clone();
        let mut chars: Vec<char> = line.chars().collect();
        chars.remove(edit.cursor_col - 1);
        edit.lines[edit.cursor_line] = chars.into_iter().collect();
        edit.cursor_col -= 1;
    } else if edit.cursor_line > 0 {
        let current = edit.lines[edit.cursor_line].clone();
        let prev_len = edit.lines[edit.cursor_line - 1].chars().count();
        edit.lines[edit.cursor_line - 1].push_str(&current);
        edit.lines.remove(edit.cursor_line);
        edit.cursor_line -= 1;
        edit.cursor_col = prev_len;
    }
    edit.dirty = true;
}

/// Split the current line at the cursor, moving the remainder onto a new line below.
fn edit_enter(edit: &mut EditState) {
    let line = edit.lines[edit.cursor_line].clone();
    let chars: Vec<char> = line.chars().collect();
    let before: String = chars[..edit.cursor_col].iter().collect();
    let after: String = chars[edit.cursor_col..].iter().collect();
    edit.lines[edit.cursor_line] = before;
    edit.lines.insert(edit.cursor_line + 1, after);
    edit.cursor_line += 1;
    edit.cursor_col = 0;
    edit.dirty = true;
}
