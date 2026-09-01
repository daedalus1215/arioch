//! View models: one frame of renderable state.
//!
//! Built from `App` by the converters in `app.rs` (`build_view`); consumed by
//! the pure render functions in `render.rs`. Render never touches `App`
//! fields or I/O — anything that needed disk (entry metadata, audit history)
//! or config (category colors) is pre-computed here.

use crate::app::{DialogState, EditState, Mode};
use crate::domain::entity::Entry;
use crate::domain::value::{Annotation, DetectedKey};
use ratatui::style::Color;
use std::path::PathBuf;

/// One frame: everything `render` needs.
pub struct View<'a> {
    pub mode: Mode,
    pub show_help: bool,
    pub sidebar_expanded: bool,
    pub sidebar_width: usize,
    pub sidebar: Sidebar<'a>,
    pub main: Main<'a>,
    pub edit: Option<Edit<'a>>,
    pub map: Map<'a>,
    pub suggestions: Suggestions<'a>,
    /// Audit history (last 20), fetched through the port by the converter.
    pub history: Vec<String>,
    pub investigate: Investigate<'a>,
    pub status: Status,
    pub dialog: Option<&'a DialogState>,
    /// The annotation shown by the AnnotView overlay, if any.
    pub annot_view: Option<&'a Annotation>,
}

pub struct Sidebar<'a> {
    pub entries: &'a [Entry],
    /// (category, entry indices) in visual order.
    pub categories: Vec<(String, Vec<usize>)>,
    pub selected_category: Option<String>,
    pub selected_entry: Option<usize>,
    pub multi_selected: &'a [usize],
}

/// The main pane (View / Search / Annotate content).
pub struct Main<'a> {
    pub selected: Option<&'a Entry>,
    /// Full entry list (search results index into it).
    pub entries: &'a [Entry],
    /// Raw selection index (search highlighting compares indices).
    pub selected_idx: Option<usize>,
    pub search_query: String,
    pub search_results: &'a [usize],
    pub file_content: Option<&'a str>,
    pub file_error: Option<&'a str>,
    pub show_diff: bool,
    pub baseline_content: Option<&'a str>,
    pub scroll_offset: usize,
    /// Pre-formatted "  Size: ..  Modified: ..  Perms: .." line, or None when
    /// the metadata read failed. The read itself happens in the converter.
    pub meta_line: Option<String>,
    pub annotations: &'a [Annotation],
    pub annot_anchor: usize,
    pub annot_cursor: usize,
    pub annot_text: Option<String>,
}

pub struct Edit<'a> {
    pub state: &'a EditState,
    /// Pre-formatted block title (" EDIT: <path> " or " EDIT ").
    pub title: String,
    /// Syntax type of the file being edited (from the selected entry's path).
    pub file_type: crate::syntax::FileType,
    pub scroll_offset: usize,
}

pub struct Map<'a> {
    pub entries: &'a [Entry],
    pub selected: Option<usize>,
    pub zoom: usize,
    pub scroll: usize,
    /// Per-entry category color (from config), same order as `entries`.
    pub colors: Vec<Color>,
}

pub struct Suggestions<'a> {
    pub items: &'a [PathBuf],
    pub filter: String,
    pub selected: usize,
    pub accepted: usize,
    pub scroll: usize,
}

pub struct Investigate<'a> {
    pub keys: &'a [DetectedKey],
    pub selected: usize,
    pub scroll: usize,
}

pub struct Status {
    pub bulk_prompt: Option<String>,
    pub bulk_input: String,
    pub multi_selected_count: usize,
    pub message: Option<String>,
    /// Pre-formatted " <n>:<path> " or " (no selection) ".
    pub entry_info: String,
}
