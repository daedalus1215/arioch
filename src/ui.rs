use crate::app::{days_to_ymd, App, Mode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, Wrap};
use ratatui::Frame;
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;


pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let bottom = chunks[1];
    let top = chunks[0];

    if top.is_empty() {
        return;
    }

    // Search mode: full screen, no sidebar
    if matches!(app.mode, Mode::Search) {
        render_main(f, top, app);
        render_status(f, bottom, app);
        if app.show_help {
            render_help(f, app);
        }
        return;
    }

    let sidebar_width = app.sidebar_width as u16;
    let (sidebar_area, main_chunk) = if app.sidebar_expanded {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sidebar_width),
                Constraint::Min(1),
            ])
            .split(top);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, top)
    };

    if let Some(area) = sidebar_area {
        render_sidebar(f, area, app);
    }

    match app.mode {
        Mode::Map => render_map(f, main_chunk, app),
        Mode::Suggestions => render_suggestions(f, main_chunk, app),
        Mode::History => render_history(f, main_chunk, app),
        Mode::Investigate => render_investigate(f, main_chunk, app),
        Mode::Edit => render_edit(f, main_chunk, app),
        Mode::Annotate => render_main(f, main_chunk, app),
        Mode::AnnotView => {
            render_main(f, main_chunk, app);
            render_annot_view(f, app);
        }
        _ => render_main(f, main_chunk, app),
    }

    render_status(f, bottom, app);

    if app.mode == Mode::Dialog {
        render_dialog(f, app);
    }

    if app.show_help {
        render_help(f, app);
    }
}

fn render_sidebar(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        " Security Files ",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let categories = app.registry.categories();
    let total = app.registry.entries.len();
    lines.push(Line::from(Span::styled(
        format!("  {} file(s)", total),
        Style::default().fg(Color::Gray),
    )));
    lines.push(Line::from(""));

    if categories.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no entries yet)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press 's' to scan",
            Style::default().fg(Color::DarkGray),
        )));
    }

    for category in &categories {
        let indices = app.registry.entries_in_category(category);
        let cat_style = if let Some(sel) = &app.selected_category {
            if sel == category {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            }
        } else {
            Style::default().fg(Color::Green)
        };

        lines.push(Line::from(Span::styled(
            format!(" ▸ {} ({})", category, indices.len()),
            cat_style,
        )));

        for &idx in &indices {
            if let Some(entry) = app.registry.get_entry(idx) {
                let is_selected = app.selected_entry == Some(idx);
                let is_multi = app.multi_selected.contains(&idx);

                let (name, style) = if is_multi {
                    (
                        format!("   ■ {}", truncate(&entry.path, app.sidebar_width - 4)),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if is_selected {
                    (
                        format!("   » {}", truncate(&entry.path, app.sidebar_width - 4)),
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (
                        format!("    {}", truncate(&entry.path, app.sidebar_width - 3)),
                        Style::default().fg(Color::DarkGray),
                    )
                };

                lines.push(Line::from(Span::styled(name, style)));

                if !entry.tags.is_empty() && is_selected {
                    let tag_str = entry.tags.join(", ");
                    lines.push(Line::from(Span::styled(
                        format!("    tags: [{}]", tag_str),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
        }
    }
    // Compute scroll to keep selected entry visible
    let visible_height = area.height.saturating_sub(2) as usize; // minus borders
    let header_lines = 4; // title, blank, count, blank
    let content_height = visible_height.saturating_sub(header_lines);

    // Find the line index of the selected entry
    let selected_line = if let Some(sel_idx) = app.selected_entry {
        lines.iter().position(|line| {
            if let Some(entry) = app.registry.get_entry(sel_idx) {
                line.to_string().contains(&truncate(&entry.path, app.sidebar_width - 4))
            } else {
                false
            }
        })
    } else {
        None
    };

    let scroll = if let Some(line_idx) = selected_line {
        if line_idx < header_lines {
            0
        } else if line_idx + content_height >= lines.len() {
            lines.len().saturating_sub(content_height)
        } else {
            line_idx.saturating_sub(header_lines)
        }
    } else {
        0
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));

    f.render_widget(paragraph, area);
}

fn render_main(f: &mut Frame, area: Rect, app: &App) {
    let title = if matches!(app.mode, Mode::Search) {
        format!(" Search: {} ", &app.search_query)
    } else if let Some(idx) = app.selected_entry {
        if let Some(entry) = app.registry.get_entry(idx) {
            if app.mode == Mode::Annotate {
                format!(" ANNOTATE: {} ", entry.path)
            } else {
                format!(" {} ", entry.path)
            }
        } else {
            " File View ".into()
        }
    } else {
        " File View ".into()
    };

    let border_color = if app.mode == Mode::Annotate {
        Color::Green
    } else {
        Color::Cyan
    };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if matches!(app.mode, Mode::Search) {
        let mut lines = Vec::new();
        // Calculate how many results fit in the available space
        let available = area.height.saturating_sub(6) as usize; // borders + header + footer
        let per_result = 2; // path line + optional tags line
        let max_visible = (available / per_result).max(1);

        if app.search_results.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  No results",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Search by: path, tags, category, description",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let total = app.search_results.len();
            let shown = total.min(max_visible);

            lines.push(Line::from(Span::styled(
                format!("  {} result(s) — showing {} ", total, shown),
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(""));

            for (i, &idx) in app.search_results.iter().take(shown).enumerate() {
                if let Some(entry) = app.registry.get_entry(idx) {
                    let is_selected = app.selected_entry == Some(idx);
                    let marker = if is_selected { "» " } else { "  " };
                    let style = if is_selected {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    lines.push(Line::from(Span::styled(
                        format!("  {}{} {}", marker, i + 1, entry.path),
                        style.clone(),
                    )));
                    if !entry.tags.is_empty() {
                        lines.push(Line::from(Span::styled(
                            format!("        tags: [{}]  cat: {}", entry.tags.join(", "), entry.category),
                            Style::default().fg(Color::Yellow),
                        )));
                    }
                }
            }

            if total > shown {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("  ... {} more", total - shown),
                    Style::default().fg(Color::DarkGray),
                )));
            }

            // Fill remaining space with blank lines
            while lines.len() < available + 4 {
                lines.push(Line::from(""));
            }

            lines.push(Line::from(Span::styled(
                "  j/k:nav  Enter:open  /:refine  Esc:back",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(Color::Black));
        f.render_widget(paragraph, area);
        return;
    }

    if let Some(idx) = app.selected_entry {
        if let Some(entry) = app.registry.get_entry(idx) {
            if app.mode == Mode::Annotate {
                render_annotate_content(f, area, app, entry, block);
                return;
            }
            let mut lines = Vec::new();

            lines.push(Line::from(Span::styled(
                format!("  Path: {}", entry.path),
                Style::default().fg(Color::Cyan),
            )));
            // File metadata
            let meta_path = expand_path_for_check(&entry.path);
            if let Ok(meta) = std::fs::metadata(&meta_path) {
                let size = human_size(meta.len());
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| {
                        let secs = d.as_secs() as i64;
                        // Simple UTC format
                        let days = secs / 86400;
                        let rem = secs % 86400;
                        let h = rem / 3600;
                        let m = (rem % 3600) / 60;
                        // Convert days to approximate date
                        let (year, month, day) = days_to_ymd(days);
                        format!("{}-{:02}-{:02} {:02}:{:02}", year, month, day, h, m)
                    })
                    .unwrap_or_else(|| "?".into());
                let perms = meta.permissions().mode() & 0o777;
                lines.push(Line::from(Span::styled(
                    format!("  Size: {}  Modified: {}  Perms: {:o}", size, mtime, perms),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            if !entry.category.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Category: {}", entry.category),
                    Style::default().fg(Color::Green),
                )));
            }
            if !entry.tags.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Tags: [{}]", entry.tags.join(", ")),
                    Style::default().fg(Color::Yellow),
                )));
            }
            if !entry.description.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Description: {}", entry.description),
                    Style::default().fg(Color::Gray),
                )));
            }
            if !entry.related.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  Related: {}", entry.related.join(", ")),
                    Style::default().fg(Color::Magenta),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  ───────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            if let Some(ref error) = app.file_error {
                lines.push(Line::from(Span::styled(
                    format!("  ERROR: {}", error),
                    Style::default().fg(Color::Red),
                )));
            } else if app.show_diff {
                // Diff view: compare baseline vs current
                let baseline = app.baseline_content.as_deref().unwrap_or("");
                let current = app.file_content.as_deref().unwrap_or("");

                if baseline == current {
                    lines.push(Line::from(Span::styled(
                        "  (no changes since first load)",
                        Style::default().fg(Color::Green),
                    )));
                } else {
                    let baseline_lines: Vec<&str> = baseline.lines().collect();
                    let current_lines: Vec<&str> = current.lines().collect();
                    let max_len = baseline_lines.len().max(current_lines.len());

                    let visible = (area.height as usize).saturating_sub(12);
                    let visible = visible.max(1);
                    let start = app.scroll_offset.min(max_len.saturating_sub(visible));

                    let mut added = 0;
                    let mut removed = 0;

                    for i in start..(start + visible).min(max_len) {
                        let b = baseline_lines.get(i).copied();
                        let c = current_lines.get(i).copied();

                        match (b, c) {
                            (None, Some(cl)) => {
                                lines.push(Line::from(Span::styled(
                                    format!("  + {}", cl),
                                    Style::default().fg(Color::Green),
                                )));
                                added += 1;
                            }
                            (Some(bl), None) => {
                                lines.push(Line::from(Span::styled(
                                    format!("  - {}", bl),
                                    Style::default().fg(Color::Red),
                                )));
                                removed += 1;
                            }
                            (Some(bl), Some(cl)) if bl != cl => {
                                lines.push(Line::from(Span::styled(
                                    format!("  - {}", bl),
                                    Style::default().fg(Color::Red),
                                )));
                                lines.push(Line::from(Span::styled(
                                    format!("  + {}", cl),
                                    Style::default().fg(Color::Green),
                                )));
                                added += 1;
                                removed += 1;
                            }
                            (Some(_), Some(_)) => {
                                lines.push(Line::from(Span::styled(
                                    format!("    {}", c.unwrap()),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                            _ => {}
                        }
                    }

                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        format!("  +{} added  -{} removed  (d:toggle diff)  [{}%]", added, removed,
                            if max_len > 0 { app.scroll_offset * 100 / max_len } else { 0 }),
                        Style::default().fg(Color::Gray),
                    )));
                }
            } else if let Some(ref content) = app.file_content {
                let total_lines = content.lines().count();
                let visible = (area.height as usize).saturating_sub(12);
                let visible = visible.max(1);

                let start = app.scroll_offset.min(total_lines.saturating_sub(visible));
                let end = (start + visible).min(total_lines);

                lines.push(Line::from(Span::styled(
                    format!("  ({} lines, showing {}-{})", total_lines, start + 1, end),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));

                let file_type = crate::syntax::detect_type(
                    &entry.path,
                    content.lines().next().unwrap_or(""),
                );

                for line in content.lines().skip(start).take(visible) {
                    let highlighted = crate::syntax::highlight_line(line, file_type);
                    let mut spans = vec![Span::raw("  ")];
                    spans.extend(highlighted.spans.iter().cloned());
                    lines.push(Line::from(spans));
                }

                if total_lines > visible {
                    let pct = (app.scroll_offset as f64 / total_lines.max(1) as f64 * 100.0) as u16;
                    lines.push(Line::from(Span::styled(
                        format!("  [{}%]  J/K:scroll  PgDn/PgUp:page  j/k:switch file", pct),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "  (no content loaded — press 'r' to refresh)",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
    } else {
        let lines = vec![
            Line::from(Span::styled(
                "  No entry selected.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press 's' to scan for security files",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Or edit ~/.config/arioch/index.toml directly",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(lines).block(block);
        f.render_widget(paragraph, area);
    }
}

fn render_edit(f: &mut Frame, area: Rect, app: &App) {
    let edit = match &app.edit {
        Some(e) => e,
        None => return,
    };

    let title = if let Some(idx) = app.selected_entry {
        if let Some(entry) = app.registry.get_entry(idx) {
            format!(" EDIT: {} ", entry.path)
        } else {
            " EDIT ".into()
        }
    } else {
        " EDIT ".into()
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let total = edit.lines.len();
    let visible = (area.height as usize).saturating_sub(3).max(1);
    let start = app.scroll_offset.min(total.saturating_sub(visible));

    let file_type = if let Some(idx) = app.selected_entry {
        if let Some(entry) = app.registry.get_entry(idx) {
            crate::syntax::detect_type(
                &entry.path,
                edit.lines.first().map(|s| s.as_str()).unwrap_or(""),
            )
        } else {
            crate::syntax::FileType::Plain
        }
    } else {
        crate::syntax::FileType::Plain
    };

    let mut lines = Vec::new();
    for (i, line) in edit.lines.iter().enumerate().skip(start).take(visible) {
        let is_cursor = i == edit.cursor_line;
        let gutter = if is_cursor { "▸" } else { " " };
        let gutter_style = if is_cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut spans = vec![Span::styled(format!(" {} ", gutter), gutter_style)];

        let mut char_offset = 0usize;
        for span in crate::syntax::highlight_line(line, file_type).spans {
            let span_len = span.content.chars().count();
            let span_start = char_offset;
            char_offset += span_len;

            if is_cursor && edit.cursor_col >= span_start && edit.cursor_col < span_start + span_len {
                let pos = edit.cursor_col - span_start;
                let chars: Vec<char> = span.content.chars().collect();
                let before: String = chars[..pos].iter().collect();
                let mid: String = chars[pos..pos + 1].iter().collect();
                let after: String = chars[pos + 1..].iter().collect();

                if !before.is_empty() {
                    spans.push(Span::styled(
                        before,
                        span.style.patch(Style::default().bg(Color::DarkGray)),
                    ));
                }
                spans.push(Span::styled(mid, span.style.add_modifier(Modifier::REVERSED)));
                if !after.is_empty() {
                    spans.push(Span::styled(
                        after,
                        span.style.patch(Style::default().bg(Color::DarkGray)),
                    ));
                }
            } else {
                let style = if is_cursor {
                    span.style.patch(Style::default().bg(Color::DarkGray))
                } else {
                    span.style
                };
                spans.push(Span::styled(span.content, style));
            }
        }
        if line.is_empty() && is_cursor {
            spans.push(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        }
        lines.push(Line::from(spans));
    }

    let mode_str = if edit.inserting {
        "INSERT"
    } else if edit.dirty {
        "MODIFIED"
    } else {
        "NORMAL"
    };
    lines.push(Line::from(Span::styled(
        format!(
            "  [{}]  Ln {} Col {}  i:insert  a:append  x:del  0:line  s:save  Esc:quit",
            mode_str,
            edit.cursor_line + 1,
            edit.cursor_col + 1
        ),
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_annotate_content(f: &mut Frame, area: Rect, app: &App, entry: &crate::registry::Entry, block: Block) {
    let content = match &app.file_content {
        Some(c) => c,
        None => {
            let lines = vec![Line::from(Span::styled(
                "  (no content loaded — press 'r' to refresh)",
                Style::default().fg(Color::DarkGray),
            ))];
            f.render_widget(Paragraph::new(lines).block(block), area);
            return;
        }
    };

    let total_lines = content.lines().count().max(1);
    let visible = (area.height as usize).saturating_sub(12).max(1);
    let start = app.scroll_offset.min(total_lines.saturating_sub(visible));

    let file_type = crate::syntax::detect_type(
        &entry.path,
        content.lines().next().unwrap_or(""),
    );

    let mut has_ann = vec![false; total_lines];
    let mut count = 0usize;
    for a in app.annotations.iter().filter(|a| a.path == entry.path) {
        count += 1;
        for l in a.start.saturating_sub(1)..a.end.min(total_lines) {
            has_ann[l] = true;
        }
    }

    let sel_start = app.annot_anchor.min(app.annot_cursor);
    let sel_end = app.annot_anchor.max(app.annot_cursor);

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            "  ({} lines, {} annotation(s), selecting {}-{})",
            total_lines,
            count,
            sel_start + 1,
            sel_end + 1
        ),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for (i, line) in content.lines().enumerate().skip(start).take(visible) {
        let is_cursor = i == app.annot_cursor;
        let is_selected = i >= sel_start && i <= sel_end;
        let gutter = if is_cursor { "▸" } else if has_ann[i] { "*" } else { " " };
        let gutter_style = if is_cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if has_ann[i] {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut spans = vec![Span::styled(format!(" {} ", gutter), gutter_style)];
        for span in crate::syntax::highlight_line(line, file_type).spans {
            let style = if is_selected {
                span.style.patch(Style::default().bg(Color::DarkGray))
            } else {
                span.style
            };
            spans.push(Span::styled(span.content, style));
        }
        if line.is_empty() && is_selected {
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    if let Some(ref t) = app.annot_text {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Comment: [{}]", t),
            Style::default().fg(Color::Cyan),
        )));
    }
    lines.push(Line::from(Span::styled(
        "  j/k:extend  v:re-anchor  c:comment  g:next  A:view  Esc:cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_annot_view(f: &mut Frame, app: &App) {
    let i = match app.annot_view {
        Some(i) => i,
        None => return,
    };
    let ann = match app.annotations.get(i) {
        Some(a) => a,
        None => return,
    };

    let area = f.area();
    let w = 48u16.min(area.width.saturating_sub(4)).max(20);
    let h = 7u16;
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    let rect = Rect::new(x, y, w, h);

    let block = Block::default()
        .title(Span::styled(
            " Annotation ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let lines = vec![
        Line::from(Span::styled(
            format!("  Lines {}-{}", ann.start, ann.end),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            format!("  {}", truncate(&ann.text, (w as usize).saturating_sub(6))),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::styled(
            format!("  created {}", ann.created),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  d:delete  Esc:close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(Color::Black));
    f.render_widget(paragraph, rect);
}

fn render_map(f: &mut Frame, area: Rect, app: &App) {
    let zoom_str = match app.map_zoom {
        0 => "",
        _ => " [detailed]",
    };
    let block = Block::default()
        .title(Span::styled(
            format!(" Relationship Map{} ", zoom_str),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let mut lines: Vec<Line> = Vec::new();

    if app.registry.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No entries to map.",
            Style::default().fg(Color::DarkGray),
        )));
        let paragraph = Paragraph::new(lines).block(block);
        f.render_widget(paragraph, area);
        return;
    }

    // Use config colors (falls back to defaults)
    let cat_color = |cat: &str| app.config.category_color(cat);

    // Build node list with resolved names
    let nodes: Vec<(usize, String, &str)> = app
        .registry
        .entries
        .iter()
        .enumerate()
        .map(|(idx, e)| {
            let name = e
                .alias
                .as_deref()
                .unwrap_or_else(|| e.path.rsplit('/').next().unwrap_or(&e.path))
                .to_string();
            (idx, name, e.category.as_str())
        })
        .collect();

    // Build name→index lookup for edge resolution
    let name_to_idx: HashMap<&str, usize> = nodes
        .iter()
        .map(|(idx, name, _)| (name.as_str(), *idx))
        .collect();

    let selected = app.map_selected;

    // Render nodes
    for (i, (idx, name, category)) in nodes.iter().enumerate().skip(app.map_scroll) {
        let is_selected = selected == Some(*idx);
        let name_display = truncate(name, 20);

        if is_selected {
            lines.push(Line::from(vec![
                Span::styled("  [*", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled(
                    name_display.clone(),
                    Style::default().fg(cat_color(category)).add_modifier(Modifier::BOLD),
                ),
                Span::styled("*]", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("  [", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    name_display.clone(),
                    Style::default().fg(cat_color(category)),
                ),
                Span::styled("]", Style::default().fg(Color::DarkGray)),
            ]));
        }

        // Detail mode: show tags
        if app.map_zoom > 0 {
            if let Some(entry) = app.registry.get_entry(*idx) {
                if !entry.tags.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("      tags: [{}]", entry.tags.join(", ")),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
        }

        // Edges (bidirectional-aware)
        if let Some(entry) = app.registry.get_entry(*idx) {
            for rel in &entry.related {
                let exists = name_to_idx.contains_key(rel.as_str());
                if exists {
                    // Check if target also lists us (bidirectional)
                    let target_idx = name_to_idx[rel.as_str()];
                    let target_entry = &app.registry.entries[target_idx];
                    let target_name = target_entry
                        .alias
                        .as_deref()
                        .unwrap_or_else(|| target_entry.path.rsplit('/').next().unwrap_or(&target_entry.path));
                    let bidirectional = target_entry.related.iter().any(|r| {
                        // Match by alias or filename
                        r == name || r == target_name
                            || r == entry.path.rsplit('/').next().unwrap_or(&entry.path)
                            || r == entry.alias.as_deref().unwrap_or("")
                    });
                    let arrow = if bidirectional { "═══▶" } else { "───▶" };
                    lines.push(Line::from(Span::styled(
                        format!("      {} {}", arrow, rel),
                        Style::default().fg(Color::Green),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("      ──✗ {} (missing)", rel),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
        }

        // Separator between nodes (compact mode)
        if i < nodes.len() - 1 + app.map_scroll {
            lines.push(Line::from(""));
        }
    }

    // Legend
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("──▶", Style::default().fg(Color::Green)),
        Span::styled(" linked   ", Style::default().fg(Color::DarkGray)),
        Span::styled("──✗", Style::default().fg(Color::Red)),
        Span::styled(" missing", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(Span::styled(
        "  +/-:zoom  j/k:nav  Enter:open  r:relayout  q:back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_suggestions(f: &mut Frame, area: Rect, app: &App) {
    let total = app.registry.suggestions.len();
    let filter_str = if app.suggestion_filter.is_empty() {
        String::new()
    } else {
        format!(" | filter: [{}]", app.suggestion_filter)
    };

    let block = Block::default()
        .title(Span::styled(
            format!(
                " Scan Suggestions ({} found, {} accepted){} ",
                total, app.suggestion_accepted, filter_str
            ),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    // Build filtered rows
    let filter = app.suggestion_filter.to_lowercase();
    let mut all_indices: Vec<usize> = Vec::new();
    for (i, p) in app.registry.suggestions.iter().enumerate() {
        let path = p.to_string_lossy().to_string();
        if !filter.is_empty() && !path.to_lowercase().contains(&filter) {
            continue;
        }
        all_indices.push(i);
    }

    // Apply scroll window
    let visible: Vec<usize> = all_indices
        .iter()
        .skip(app.suggestion_scroll)
        .take(14)
        .cloned()
        .collect();

    let mut rows = Vec::new();
    for &i in &visible {
        let p = &app.registry.suggestions[i];
        let path = p.to_string_lossy().to_string();
        let filename: String = p
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());

        let is_selected = i == app.suggestion_selected;
        let marker = if is_selected { "» " } else { "  " };

        let row_style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        rows.push(Row::new(vec![
            Span::styled(format!("{}{}", marker, path), row_style.clone()),
            Span::styled(filename, row_style),
        ]));
    }

    let widths = [Constraint::Percentage(70), Constraint::Percentage(30)];
    let table = Table::new(rows, widths).header(
        Row::new(vec!["Path", "File"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    );

    let table = table.block(block);
    f.render_widget(table, area);
}
fn render_history(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Span::styled(
            " Audit History (last 20) ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let entries = app.read_history();

    let mut lines = Vec::new();
    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No history recorded yet.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for entry in &entries {
            // Color-code by action
            let style = if entry.contains(" add ") {
                Style::default().fg(Color::Green)
            } else if entry.contains(" remove ") {
                Style::default().fg(Color::Red)
            } else if entry.contains(" edit ") {
                Style::default().fg(Color::Yellow)
            } else if entry.contains(" scan ") {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Reset)
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", entry),
                style,
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  q/Esc: back to view",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}


fn render_investigate(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(Span::styled(
            " Investigate — Config Explanations ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines: Vec<Line> = Vec::new();

    if app.investigate_keys.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No keys detected in this file.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  This file type may not be supported for investigation.",
            Style::default().fg(Color::DarkGray),
        )));
        let paragraph = Paragraph::new(lines).block(block);
        f.render_widget(paragraph, area);
        return;
    }

    // Summary header
    let known = app.investigate_keys.iter().filter(|k| k.entry.is_some()).count();
    lines.push(Line::from(Span::styled(
        format!("  {} keys detected, {} with explanations", app.investigate_keys.len(), known),
        Style::default().fg(Color::Gray),
    )));
    lines.push(Line::from(""));

    // Render visible keys
    for (i, dk) in app
        .investigate_keys
        .iter()
        .enumerate()
        .skip(app.investigate_scroll)
        .take(10)
    {
        let is_selected = i.saturating_sub(app.investigate_scroll) == app.investigate_selected.saturating_sub(app.investigate_scroll);

        // Key name line
        let section_str = dk
            .section
            .as_deref()
            .map(|s| format!(" [{}]", s))
            .unwrap_or_default();

        if is_selected {
            lines.push(Line::from(vec![
                Span::styled("  » ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("L{} {}{}", dk.line + 1, dk.key, section_str),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  =  {}", truncate(&dk.value, 30)),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(
                    format!("L{} {}{}", dk.line + 1, dk.key, section_str),
                    Style::default().fg(Color::Reset),
                ),
                Span::styled(
                    format!("  =  {}", truncate(&dk.value, 30)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        // 3-line card if we have knowledge
        if let Some(ref ke) = dk.entry {
            let danger_color = match ke.danger {
                crate::knowledge::Danger::Safe => Color::Green,
                crate::knowledge::Danger::Caution => Color::Yellow,
                crate::knowledge::Danger::Dangerous => Color::Red,
            };
            let danger_label = match ke.danger {
                crate::knowledge::Danger::Safe => "safe",
                crate::knowledge::Danger::Caution => "caution",
                crate::knowledge::Danger::Dangerous => "DANGER",
            };

            lines.push(Line::from(Span::styled(
                format!("      what: {}", ke.what),
                Style::default().fg(Color::Reset),
            )));
            lines.push(Line::from(Span::styled(
                format!("      why:  {}", ke.why),
                Style::default().fg(danger_color),
            )));
            lines.push(Line::from(Span::styled(
                format!("      how:  {}", ke.how),
                Style::default().fg(Color::Cyan),
            )));
            if ke.danger != crate::knowledge::Danger::Safe {
                lines.push(Line::from(Span::styled(
                    format!("      ⚠ {}", danger_label),
                    Style::default().fg(danger_color).add_modifier(Modifier::BOLD),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "      (no explanation — add to ~/.config/arioch/knowledge.toml)",
                Style::default().fg(Color::DarkGray),
            )));
        }

        // Separator
        if i < app.investigate_keys.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    // Footer
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  j/k:navigate  PgDn/PgUp:page  q/Esc:back",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}
fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let mode_str = match app.mode {
        Mode::View => "VIEW",
        Mode::Map => "MAP",
        Mode::Suggestions => "SCAN",
        Mode::Search => "SEARCH",
        Mode::Dialog => "DIALOG",
        Mode::History => "HISTORY",
        Mode::Investigate => "INVESTIGATE",
        Mode::Edit => "EDIT",
        Mode::Annotate => "ANNOTATE",
        Mode::AnnotView => "NOTE",
    };

    // Bulk prompt takes priority
    if let Some(ref prompt) = app.bulk_prompt {
        let text = vec![
            Span::styled(
                format!(" {} | {} {} ", mode_str, prompt, app.bulk_input),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        let paragraph = Paragraph::new(Line::from(text)).style(Style::default().fg(Color::Black));
        f.render_widget(paragraph, area);
        return;
    }

    let help = if app.mode == Mode::Edit {
        "i:insert  a:append  x:del  0:line  s:save  Esc:quit".to_string()
    } else if app.mode == Mode::Annotate {
        "j/k:extend  v:re-anchor  c:comment  g:next  A:view  Esc:cancel".to_string()
    } else if app.mode == Mode::AnnotView {
        "d:delete  Esc:close".to_string()
    } else if !app.multi_selected.is_empty() {
        format!(
            "{} selected | Space:toggle  t:tag  C:categorize  D:remove  Esc:clear",
            app.multi_selected.len()
        )
    } else {
        "j/k:nav  h/l:sidebar  m:map  s:scan  /:search  E:inline-edit  v:annotate  e:$EDITOR  x:rm  q:quit"
            .to_string()
    };

    let msg = if let Some(ref m) = app.message {
        m.as_str()
    } else {
        &help
    };

    let entry_info = if let Some(idx) = app.selected_entry {
        if let Some(entry) = app.registry.get_entry(idx) {
            format!(" {}:{} ", idx + 1, entry.path)
        } else {
            " ".into()
        }
    } else {
        " (no selection) ".into()
    };

    let text = vec![
        Span::styled(
            format!(" {} |{}| ", mode_str, entry_info),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(msg, Style::default().fg(Color::Gray)),
    ];

    let paragraph = Paragraph::new(Line::from(text)).style(Style::default().fg(Color::Black));
    f.render_widget(paragraph, area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - max + 3..])
    }
}

fn render_dialog(f: &mut Frame, app: &App) {
    let dialog = match &app.dialog {
        Some(d) => d,
        None => return,
    };

    let full_area = f.area();
    let dialog_width = 50u16;
    let dialog_height = 12u16;

    let x = full_area.x + (full_area.width.saturating_sub(dialog_width)) / 2;
    let y = full_area.y + (full_area.height.saturating_sub(dialog_height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };

    let title = if dialog.is_new {
        " Add Entry "
    } else {
        " Edit Entry "
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = Vec::new();

    let fields: [(&str, &String, bool); 6] = [
        ("Path: ", &dialog.path, true),
        ("Category: ", &dialog.category, true),
        ("Tags: ", &dialog.tags, true),
        ("Desc: ", &dialog.description, true),
        ("Alias: ", &dialog.alias, true),
        ("Related: ", &dialog.related, true),
    ];

    for (i, (label, value, _)) in fields.iter().enumerate() {
        let is_current = dialog.current_field == i;

        let label_style = if is_current {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let cursor = if is_current { "▌" } else { "" };
        let value_style = if is_current {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Reset)
        };

        // Check for warnings
        let warning = String::new();
        if i == 0 && !value.is_empty() {
            let path = expand_path_for_check(value);
            if !std::path::Path::new(&path).exists() {
                let _ = &warning; // suppress unused warning
            }
        }
        if i == 5 && !value.is_empty() {
            for rel in value.split(',') {
                let rel = rel.trim();
                if !rel.is_empty() {
                    let exists = app.registry.entries.iter().any(|e| {
                        e.alias.as_deref() == Some(rel)
                            || e.path.rsplit('/').next() == Some(rel)
                    });
                    if !exists {
                        break;
                    }
                }
            }
        }

        lines.push(Line::from(vec![
            Span::styled(label.to_string(), label_style),
            Span::styled(
                format!("{}{}", value, cursor),
                value_style,
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Tab:next  Enter:save  Esc:cancel  Up/Down:field",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, dialog_area);
}

fn expand_path_for_check(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}

fn render_help(f: &mut Frame, _app: &App) {
    let area = f.area();
    let width = (area.width / 2).min(50).max(30);
    let height = (area.height / 2).min(32).max(24);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let block = Block::default()
        .title(Span::styled(
            " Keybindings ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let lines = vec![
        Line::from(Span::styled("  Navigation", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  j / Down       next entry"),
        Line::from("  k / Up         prev entry"),
        Line::from("  h / Left       collapse sidebar"),
        Line::from("  l / Right      expand sidebar"),
        Line::from("  + / -          line cursor (view)"),
        Line::from(""),
        Line::from(Span::styled("  Actions", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  e              open in $EDITOR"),
        Line::from("  E              inline edit (vim-like)"),
        Line::from("  v              annotate selection"),
        Line::from("  A              view annotation at line"),
        Line::from("  g              next annotation"),
        Line::from("  d              toggle diff view"),
        Line::from("  r              refresh file"),
        Line::from("  a              add entry"),
        Line::from("  x              remove entry"),
        Line::from("  s              scan for files"),
        Line::from("  c              copy path to clipboard"),
        Line::from(""),
        Line::from(Span::styled("  Views", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  m              relationship map"),
        Line::from("  H              audit history"),
        Line::from("  i              investigate config keys"),
        Line::from("  /              search"),
        Line::from(""),
        Line::from(Span::styled("  Misc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from("  ?              toggle this help"),
        Line::from("  Esc / q        back / quit"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(Color::Black));
    f.render_widget(paragraph, popup);
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
