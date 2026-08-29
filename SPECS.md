# arioch — Feature Specs

Each spec is self-contained: implement one, verify, move to next.
Status: `[ ]` = not started, `[~]` = in progress, `[x]` = done.

---

## 1. Add Entry Dialog

Status: `[x]`

### Goal
Press `a` opens an inline TUI form to register a new security file without leaving the app.

### UI Layout

```
┌──────────────────────────────────────────────────────┐
│  Add Entry                                            │
├──────────────────────────────────────────────────────┤
│  Path:      [~/]                                      │
│  Category:  [ssh-keys          ]                      │
│  Tags:      [deploy, prod      ]                      │
│  Desc:      [Primary deploy key]                      │
│  Alias:     [id_ed25519        ]                      │
│  Related:   [aws-creds         ]                      │
├──────────────────────────────────────────────────────┤
│  Tab: next field  Enter: save  Esc: cancel            │
└──────────────────────────────────────────────────────┘
```

### Behavior
- **Path** — first field, pre-filled with `~/`. Tab or Enter to confirm.
  - If path exists on disk, auto-detect category from path heuristics (same `guess_category` logic).
  - If path doesn't exist, allow it but show a warning indicator.
- **Category** — text field. Tab to advance.
- **Tags** — comma-separated. Tab to advance.
- **Description** — single-line text. Tab to advance.
- **Alias** — optional. Tab to advance.
- **Related** — comma-separated aliases/paths of existing entries. Tab to advance.
  - Show hint if related entry not found: `Related: [aws-creds (missing)]`
- **Enter** on any field — save entry, close dialog, refresh sidebar.
- **Esc** — cancel, no changes saved.
- **Backspace** — delete character in current field.
- **Tab** — advance to next field. Shift+Tab — go back.

### Data
- New entry appended to `registry.entries`.
- Saved immediately to `index.toml`.
- `selected_entry` set to the new entry's index.
- Content loaded via `refresh_content()`.

### Edge Cases
- Duplicate path: if an entry with same path exists, update it instead of adding (show "Updated" in status).
- Empty path: reject with status message "Path is required".
- Very long path: truncate display in sidebar (already handled by `truncate()`).

### Acceptance
- `a` → dialog appears, all fields navigable with Tab/Shift+Tab
- Enter a valid path + category → entry appears in sidebar, saved to TOML
- Esc → dialog closes, no entry added
- Enter a path that already exists → existing entry updated, status shows "Updated"
- Enter a related alias that doesn't exist → entry still saved, map view shows red arrow

---

## 2. Change Entry Fields (Edit Dialog)

Status: `[x]`

### Goal

### UI Layout
Same as Add Entry dialog, but:
- Title: `Edit Entry: <alias or filename>`
- All fields pre-populated from current entry
- Enter → save changes, close dialog
- Esc → discard changes, close dialog
- `d` key in dialog → delete entry (replaces the top-level `d` behavior)

### Behavior
- Same field navigation as Add Entry.
- On save: update entry in place (not append), save to TOML, refresh content.
- On delete (`d`): remove entry, confirm with status message, close dialog.
- If path field is changed: re-load file content for the new path.

### Acceptance
- `c` → dialog pre-filled with selected entry's values
- Change category → save → sidebar updates category grouping
- Change path → save → main panel loads new file content
- `d` in dialog → entry removed, saved to TOML
- Esc → no changes made

---

## 3. File Type Detection & Syntax Coloring

Status: `[x]`

### Goal
Auto-detect file type from extension/content and apply basic syntax highlighting in the viewer.

### Supported Types & Color Rules

| Type | Extensions | Colors |
|------|-----------|--------|
| TOML | `.toml`, `.cfg`, `Cargo.toml` | Keys: Cyan, Strings: Green, Comments: Gray, Booleans: Yellow, Numbers: Magenta |
| JSON | `.json` | Keys: Cyan, Strings: Green, Numbers: Magenta, Booleans/Null: Yellow, Comments: N/A |
| YAML | `.yaml`, `.yml` | Keys: Cyan, Strings: Green, Comments: Gray, Anchors: Magenta |
| INI/Config | `.conf`, `.ini`, `ssh_config`, `known_hosts` | Sections: Green, Keys: Cyan, Values: Reset, Comments: Gray |
| SSH | `id_*`, `authorized_keys`, `known_hosts`, `config` (in ~/.ssh) | Same as INI |
| Env | `.env`, `*.env` | Keys: Cyan, Values: Green, Comments: Gray, `export`: Magenta |
| PEM/Cert | `.pem`, `.crt`, `.key`, `.p12` | `-----BEGIN/END`: Green bold, Base64: DarkGray |
| Shell | `.sh`, `.bash` | Comments: Gray, Keywords (if/for/while/function): Magenta, Strings: Green, `$VAR`: Yellow |
| Plain text | Everything else | No highlighting |

### Detection Logic
1. Check extension (lowercase) against known set.
2. If extension matches multiple (e.g., `config`), check parent directory (`~/.ssh/config` → SSH, `~/.config/app/config` → INI).
3. If no extension match, check first line for `-----BEGIN` → PEM.
4. Fallback: no highlighting.

### Implementation
- New module: `src/syntax.rs`
- Function: `highlight_line(line: &str, file_type: FileType) -> Vec<Span>`
- In `render_main()`: after reading file content, detect type, then render each line via `highlight_line`.
- Only highlight the visible window (not the whole file).

### Acceptance
- Open a TOML file → keys in cyan, strings in green, comments in gray
- Open a `.pem` file → BEGIN/END markers green bold, base64 in gray
- Open a `.env` file → keys in cyan, values in green, `export` in magenta
- Open a plain `.txt` → no highlighting (default color)
- Open `~/.ssh/config` → INI-style highlighting

---

## 4. Extended Map View

Status: `[x]`

### Goal
Improve the relationship map from a flat list to a proper ASCII graph with spatial layout.

### UI Layout

```
┌─ Relationship Map ────────────────────────────────────┐
│                                                       │
│   [id_ed25519] ──────► [ssh-config]                   │
│        │                   │                           │
│        │                   ▼                           │
│        │             [known_hosts]                      │
│        ▼                                               │
│   [aws-creds] ─────────► [hosts]                       │
│                                                       │
│   Legend:  ──► exists    ┄┄► missing                   │
│   Filter:  all  [ssh-keys]  [cloud-creds]              │
│                                                       │
│   Press [1-9] to highlight a node, [0] to clear       │
│   Press [f] to filter by category                     │
└───────────────────────────────────────────────────────┘
```

### Behavior
- **Layout**: Simple topological sort. Nodes with no incoming edges on top, edges flow downward. Max 3 columns.
- **Node format**: `[alias]` or `[filename]`, colored by category:
  - ssh-keys: Cyan
  - cloud-creds: Magenta
  - system-config: Green
  - certificates: Yellow
  - other: Reset
- **Edges**: `───►` for existing targets, `┄┄┄►` (dashed) for missing.
- **Highlight**: Press `1`-`9` to highlight node N (bold + border). Press `0` to clear.
- **Filter**: Press `f` to cycle through categories, showing only that category's nodes + their connections.
- **Missing nodes**: Shown in parentheses, red.
- **Self-loops**: Shown as `[node] ──► [node] (self)` on same line.
- **Cycles**: Detected and shown with `↻` suffix on the node.

### Data
- Build directed graph from `related` fields.
- Topological sort (Kahn's algorithm). Cycles handled by breaking them.
- Layout: BFS from roots, assign (row, col) positions.

### Acceptance
- 3+ entries with `related` links → rendered as a graph with proper node/edge positions
- Press `1` → first node highlighted
- Press `f` → filter to ssh-keys only, other nodes hidden
- Press `0` → clear highlight
- A node with a missing `related` target → dashed edge, target shown in red
- A cycle (A→B→A) → `↻` shown on both nodes, no infinite loop

---

## 5. Scan Results Navigation & Filter

Status: `[x]`

### Goal
The scan suggestions view gets proper keyboard navigation, filtering, and bulk operations.

### Current State
- `s` triggers scan, shows a table of suggestions.
- `a` accepts all, `i` accepts first, `q`/`Esc` closes.

### Improved Behavior
- **Navigation**: `j`/`k` or arrow keys to move through suggestions. Selected row highlighted.
- **Filter**: Type to filter suggestions by path substring (like search mode).
- **Accept selected**: `a` accepts the *selected* row (not all). `A` (shift) accepts all.
- **Reject**: `d` removes selected from suggestions (hide it).
- **Preview**: Enter or `e` opens the selected file in the viewer (read-only, no add).
- **Status bar**: Shows `n/total` suggestions, `m` accepted, `k` rejected.
- **Re-scan**: `r` re-runs scan (in case files changed).

### UI

Status: `[x]`
┌─ Scan Suggestions (23 found, 2 accepted) ─────────────┐
│  Filter: [ssh]                                         │
├────────────────────────────────────────────────────────┤
│  » ~/.ssh/id_ed25519.pub                               │
│    ~/.ssh/known_hosts                                  │
│    ~/.ssh/config                                       │
│    /etc/ssh/sshd_config                                │
│    ~/.ssh/id_rsa                                       │
├────────────────────────────────────────────────────────┤
│  j/k:navigate  a:accept  A:all  d:reject  e:preview   │
│  /:filter  r:rescan  q:quit                            │
└────────────────────────────────────────────────────────┘
```

### Acceptance
- `s` → scan runs, suggestions appear with first row selected
- `j`/`k` → move selection up/down
- Type `ssh` → filter narrows to ssh-related suggestions
- `a` → accepts selected, it disappears from list, count updates
- `A` → accepts all, view closes
- `d` → rejects selected, it's hidden
- `e` → opens selected file in main viewer (temporarily, `q` returns to scan)
- `r` → re-scans, refreshes list
- Filter cleared (Esc or backspace to empty) → all suggestions shown

---

## 6. Multi-Selection & Bulk Operations

Status: `[x]`

### Goal
Select multiple entries and perform bulk operations (tag, categorize, remove).

### Behavior
- **Toggle select**: `Space` toggles selection on the current entry.
- **Select all**: `Ctrl+A` selects all entries.
- **Select category**: In sidebar, `Shift+Enter` selects all entries in the current category.
- **Deselect all**: `Esc` clears multi-selection.
- **Bulk remove**: `D` (shift) removes all selected (with confirm: "Remove N entries? y/n").

### UI
- Selected entries in sidebar: `»` becomes `■` (filled square).
- Status bar shows: `N selected | j/k:navigate  Space:toggle  t:tag  C:categorize  D:remove`
- Confirmation for destructive ops: status bar shows prompt, `y`/`n` to confirm/cancel.

### Acceptance
- `Space` on entry → `»` becomes `■` in sidebar
- `Space` on two entries → both highlighted
- `t` → status bar shows "Add tag to 2 entries: [input]"
- Type `production` + Enter → both entries get the tag, saved to TOML
- `C` → status bar shows "Set category for 2 entries: [input]"
- `D` → "Remove 2 entries? y/n" → `y` → entries removed
- `Esc` → clears selection, normal navigation resumes

---

## 7. File Watch & Auto-Refresh

Status: `[x]`

### Goal
Detect when a file changes on disk and auto-refresh the viewer.

### Behavior
- Poll the currently-viewed file's modification time every 2 seconds (simple, no `notify` crate).
- If mtime changed: re-read content, reset scroll offset, flash status: "File updated: <path>".
- If file was deleted: show error state: "File no longer exists: <path>".
- If file content exceeds 1MB: show first 1MB with warning.
- Ctrl+R: force refresh regardless of mtime.

### Implementation
- In the tick handler: if `mode == View` and `selected_entry` is set, check mtime.
- Store last-seen mtime in `App` state.
- Use `std::fs::metadata().modified()` for the check.

### Acceptance
- Open a file, `nano` the file externally, save → arioch auto-refreshes within 2s
- Delete the file → status shows "File no longer exists"
- Restore the file → auto-refresh shows content again
- Ctrl+R → forces re-read even if mtime unchanged

---

## 8. Entry History & Audit Log

Status: `[ ]`

### Goal
Status: `[x]`

### Storage
- `~/.config/arioch/history.log` — append-only text file.
- Format: `<ISO8601 timestamp> <action> <path> <details>`
- Actions: `add`, `remove`, `edit`, `scan`, `bulk-tag`, `bulk-categorize`

### Behavior
- Every mutation to the registry appends a line to the log.
- `h` (in view mode) → shows the last 20 log entries in the main panel (replaces file view temporarily).
- `q` or `Esc` → returns to normal view.
- Log is not editable via the TUI (view only).

### Example Log
```
2026-08-26T14:32:01Z add ~/.ssh/id_ed25519 category=ssh-keys tags=[deploy,production]
2026-08-26T14:32:15Z add ~/.ssh/config category=ssh-keys
2026-08-26T14:33:02Z edit ~/.ssh/id_ed25519 related=[aws-creds,known_hosts]
2026-08-26T14:33:45Z scan 23 suggestions found, 3 accepted
2026-08-26T14:34:10Z remove /etc/shadow reason=user-requested
```

### Acceptance
- Add an entry → line appended to history.log
- Remove an entry → line appended with action=remove
- Press `h` → last 20 entries shown in main panel
- Press `q` → returns to file view
- Log file is append-only (never truncated by arioch)

---

## 9. CLI Subcommands

Status: `[x]`

### Goal
Non-interactive CLI operations for scripting and automation.

### Commands

```
arioch                        # Launch TUI (default)
arioch list                   # List all entries (path, category, tags)
arioch list --category SSH    # Filter by category
arioch list --json            # Output as JSON
arioch add <path> [--category C] [--tags t1,t2] [--alias A] [--related r1,r2]
arioch remove <path>
arioch scan                   # Run scan, print suggestions
arioch scan --accept          # Accept all suggestions
arioch info <path>            # Show metadata for a single entry
arioch map                    # Print ASCII map to stdout
arioch map --missing          # Only show entries with missing related refs
arioch path <alias>           # Resolve alias to full path
arioch version                # Print version
```

### Behavior
- All commands operate on the same `~/.config/arioch/index.toml`.
- `add` with existing path → updates (idempotent).
- `remove` prints confirmation to stdout (no prompt, for scripting).
- `map` outputs the same ASCII graph as the TUI.
- `--json` flag for machine-readable output.
- Exit codes: 0 success, 1 not found, 2 invalid args.

### Acceptance
- `arioch list` → prints all entries with path, category, tags
- `arioch list --category ssh-keys` → filtered output
- `arioch add ~/.ssh/id_ed25519 --category ssh-keys --tags deploy` → adds entry
- `arioch add ~/.ssh/id_ed25519 --category ssh-keys` (again) → updates, no duplicate
- `arioch remove ~/.ssh/id_ed25519` → removes, prints confirmation
- `arioch info ~/.ssh/id_ed25519` → prints all metadata
- `arioch path id_ed25519` → prints `/home/user/.ssh/id_ed25519`
- `arioch list --json` → valid JSON array
- `arioch map` → ASCII graph in stdout

---

## 10. Configuration File

Status: `[x]`

### Goal
Move hardcoded defaults to a user-editable config.

### File
`~/.config/arioch/config.toml`

```toml
# Scan paths to search for security files
scan_paths = [
    "~/.ssh",
    "~/.config",
    "/etc/ssh",
    "/etc/ssl",
    "~/.gnupg",
    "~/.aws",
    "~/.kube",
    "~/.docker",
]

# Glob patterns for file matching
scan_patterns = [
    "*.pub",
    "id_*",
    "known_hosts",
    "*.pem",
    "*.crt",
    "*.key",
    "*.p12",
    "*.pfx",
    "*.conf",
    "*.toml",
    "*.yaml",
    "*.yml",
    "*.json",
    "hosts",
    "shadow",
    "sudoers",
    "*.env",
    ".env*",
    "*.secret",
    "*.credentials",
]

# Max directory depth for scanning
scan_depth = 5

# Max file size to display (bytes, default 1MB)
max_file_size = 1048576

# Auto-refresh poll interval (seconds, 0 = disabled)
refresh_interval = 2

# Default editor (overrides $EDITOR)
# editor = "vim"

# Category color overrides
[colors]
ssh-keys = "cyan"
cloud-creds = "magenta"
system-config = "green"
certificates = "yellow"
```

### Behavior
- If config file doesn't exist, use built-in defaults (current behavior).
- If config file exists, parse and merge with defaults.
- Invalid config → warning to stderr, use defaults.
- `editor` field overrides `$EDITOR` env var.
- `scan_depth` replaces the hardcoded `5` in `walkdir_simple`.
- `max_file_size` caps file content loading.
- `refresh_interval` controls the poll timer (0 = disabled).
- `colors` table maps category names to color names.

### Acceptance
- No config file → uses defaults (current behavior)
- Create config with custom `scan_paths` → scan only searches those paths
- Set `editor = "vim"` → pressing `e` opens vim instead of `$EDITOR`
- Set `refresh_interval = 0` → no auto-refresh
- Invalid TOML in config → warning, defaults used
- Set `colors.ssh-keys = "yellow"` → ssh-keys entries render in yellow

---

## 11. Inline File Editing

Status: `[x]`

### Goal

Edit file content directly inside the TUI without launching `$EDITOR`. A vim-lite inline editor covers small, surgical edits (rotating a key path, fixing a value) with a safe save flow.

### Entry

- Press `E` (shift) in view mode with file content loaded → enter inline edit mode.
- Guards (status message, no mode change):
  - No content loaded / read error → "Cannot edit: no content loaded — press 'r' to refresh"
  - File not writable (permission bits) → "File is read-only (mode 0oNNN) — use external editor (e)"

### Edit Mode Keys

| Key | Action |
|-----|--------|
| `h/j/k/l`, arrows | Move cursor (line/char) |
| `i` | Insert before cursor |
| `a` | Insert after cursor |
| `A` | Insert at end of line |
| `0` / `$` | Line start / line end |
| `Backspace` | Delete char before cursor (merges with previous line at start) |
| `Delete` | Delete char under cursor (merges with next line at end) |
| `x` | Delete char under cursor |
| `Enter` | Split line at cursor |
| `o` / `O` | New line below / above, enter insert |
| `PgUp` / `PgDn` | Move cursor a page |
| `s` | Save to disk and exit |
| `Esc` | In insert mode: exit insert mode. In normal mode: if dirty, prompt `y` save / `n` discard / `Esc` keep editing; if clean, exit |
| `q` | Same as `Esc` in normal mode (save prompt or exit). In insert mode, types a literal `q` |
- In insert mode every printable key is text (vim-faithful); command keys like `i`, `a`, `s` are characters.

- Insert mode shows `INSERT` in the status bar (green); normal shows `EDIT`.
- Cursor position shown as `Ln N Col M`.
- `Esc` on a clean buffer exits immediately.

### Data

- Edit buffer is an in-memory clone of `file_content` (lines + trailing-newline flag).
- On save: write file, update `file_content` and `baseline_content` (diff view reflects pre-edit state), append `edit <path> inline` to history log.
- On discard: reload file from disk, message "Changes discarded".
- Save failure (permissions) → status error, stay in view mode, buffer kept in `file_content` (diff view `d` shows what would be written).

### Acceptance

- `E` → cursor visible on first line, status shows `EDIT` and `Ln 1 Col 1`
- `i` → `INSERT`, typed chars appear at cursor
- `s` → file on disk updated, status "Saved", diff view (`d`) shows no changes
- `Esc` with unsaved changes → prompt; `n` → file unchanged on disk, content reloaded
- `Esc` with no changes → exits immediately
- Read-only file → `E` shows mode message, stays in view
- Edit a `.toml` entry → syntax highlighting preserved in edit mode

---

## 12. Inline Annotations (Plannotator-style)

Status: `[x]`

### Goal

Select a range of lines in the file viewer and attach a persistent comment — the "Herdr Annotate" effect: highlight the selected lines, open a small comment popover next to them, save the note. Gutter markers show annotated lines; jump and review notes without leaving the viewer.

### Storage

`~/.config/arioch/annotations.toml` (respects `--config`):
[[annotations]]
path = "~/.ssh/config"
start = 12
end = 14
text = "Rotate this key quarterly"
created = "2026-08-29T10:00:00Z"
```

- Line numbers are 1-based, inclusive.
- Keyed by the entry's registered path string (gutter matches it against the entry, not the disk path).

### Keys (view mode)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move the line cursor (file switching stays on `j`/`k`) |
| `v` | Start selection at the line cursor |
| `j/k`, `↑/↓` (in selection) | Extend selection |
| `c` (in selection) | Open comment popover |
| `Esc` (in selection) | Cancel selection |
| `g` | Jump to next annotation (wraps), scrolls to it |
| `A` | View annotation at line cursor (read-only popover) |
| `d` (in annotation popover) | Delete that annotation |

### UI

- Selected lines: full-line background highlight + `▌` gutter marker (like the video).
- Popover: small bordered box anchored near the selection (right of center, clamped in bounds): title `Annotate lines X–Y`, single-line text input, `Enter:save  Esc:cancel`.
- Annotation view popover: title `Annotation L X–Y`, wrapped text body, `Esc:close  d:delete`.
- Gutter: `●` (yellow) on any line covered by an annotation, in view, edit, and selection modes.
- Status bar in selection: `lines X–Y selected | c:comment  Esc:cancel`.

### Data

- Save appends the annotation, persists immediately, status "Annotated lines X–Y".
- Delete removes it, persists, status "Annotation removed".
- Audit: `annotate <path> lines=X-Y` / `unannotate <path> lines=X-Y` in history log.
- Empty comment text → popover stays open.

### Acceptance

- `v` then `j j j` then `c` → 4 lines highlighted, popover appears
- Type text + Enter → `annotations.toml` contains the range, status confirms
- Re-open the file → `●` gutter markers on the annotated lines
- `g` → scrolls to the annotation; `A` on that line → popover shows the text
- `d` in popover → annotation removed from file, status confirms
- `Esc` in selection → highlight cleared, back to view
