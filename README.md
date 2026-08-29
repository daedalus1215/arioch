# arioch

Security file manager. View, manage, and map local security files from a single terminal UI — no network, no browser, no runtime.

## What it does

- **Registers** security files scattered across your system (SSH keys, configs, credentials, certs, deployment files) into one index
- **Views** file content with syntax highlighting (TOML, JSON, YAML, INI, SSH config, PEM, shell, env)
- **Maps** relationships between files as a navigable graph
- **Scans** directories to discover security files you haven't registered yet
- **Edits** inline in the TUI (vim-style normal/insert modes) or via your `$EDITOR`
- **Annotates** files with inline comments (plannotator-style): select a line range, attach a note, see gutter markers, jump between annotations
- **Watches** files for changes and auto-refreshes
- **Audits** every mutation to an append-only log
- **Scriptable** — all operations available as non-interactive CLI subcommands

## Install

```bash
cd ~/Projects/arioch
cargo install --path .
```

Requires Rust 1.75+. No other runtime dependencies.

## Usage

### TUI

```bash
arioch
```

Opens the full terminal UI.

### CLI

```bash
arioch list                          # list all entries
arioch add ~/.ssh/id_ed25519 --category ssh-keys --tags "deploy,prod"
arioch remove ~/.ssh/id_ed25519
arioch tag ~/.ssh/id_ed25519 critical
arioch map                           # print relationship graph
arioch scan                          # discover unregistered security files

# JSON output (for scripting)
arioch list --json
arioch map --json

# Custom config location
arioch --config /path/to/dir list
```

## Keybindings (TUI)

| Key | Action |
|-----|--------|
| `j` / `↓` | Next file (sidebar navigation) |
| `k` / `↑` | Previous file |
| `J` / `K` | Scroll content down/up (within current file) |
| `PgDn` / `PgUp` | Page scroll content |
| `h` / `←` | Collapse sidebar |
| `l` / `→` | Expand sidebar |
| `m` | Toggle relationship map |
| `s` | Scan for security files |
| `/` | Search across all entries |
| `a` | Add new entry (dialog) |
| `E` | Inline edit (vim-style, in TUI) |
| `e` | Open file in `$EDITOR` |
| `v` | Start annotation selection |
| `g` (in view) | Jump to next annotation |
| `d` | File diff view (baseline vs current) |
| `x` | Delete selected entry
| `r` | Force refresh file content |
| `H` | Audit history (last 20 changes) |
| `Space` | Toggle multi-select |
| `t` (with selection) | Bulk add tag |
| `C` (with selection) | Bulk set category |
| `D` (with selection) | Bulk remove |
| `Esc` | Clear selection / go back |
| `q` | Quit |

### Map view keys

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate between nodes |
| `+` / `=` | Zoom in (show tags) |
| `-` | Zoom out (compact) |
| `Enter` | Open selected node's file |
| `r` | Re-layout |
| `q` / `Esc` | Back to view |

### Inline edit mode keys

| Key | Action |
|-----|--------|
| `i` / `a` / `A` | Insert at cursor / after cursor / end of line |
| `h` / `j` / `k` / `l` | Move cursor |
| `0` / `^` | Start of line / first non-blank |
| `$` | End of line |
| `x` / `Backspace` | Delete char under / before cursor |
| `o` / `O` | New line below / above |
| `s` | Save and exit |
| `Esc` | Exit insert mode; then save prompt / quit

## Configuration

All files live in `~/.config/arioch/`:

| File | Purpose |
|------|---------|
| `config.toml` | Scan paths, patterns, depth, file size limit, refresh interval, editor, colors |
| `index.toml` | Registered entries (path, category, tags, description, alias, related) |
| `history.log` | Append-only audit log of all mutations |
| `annotations.toml` | Inline file annotations (line ranges + comments)

### Example config.toml

```toml
scan_paths = ["~/.ssh", "~/.config", "/etc"]
scan_patterns = ["id_*", "*.pem", "*.crt", "*.key", "credentials", "secrets*", "*.env", ".env.*"]
scan_depth = 3
max_file_size = 1048576
refresh_interval = 2
# editor = "nvim"  # defaults to $EDITOR, then nano
```

### Example index.toml

```toml
[[entries]]
path = "~/.ssh/id_ed25519"
category = "ssh-keys"
tags = ["deploy", "production"]
description = "Primary deploy key"
alias = "id_ed25519"
related = ["aws-creds", "known_hosts"]

[[entries]]
path = "~/.config/aws/credentials"
category = "cloud-creds"
tags = ["aws", "production"]
alias = "aws-creds"
related = ["id_ed25519"]
```

## Project structure

```
arioch/
├── Cargo.toml
├── SPECS.md          # feature specs (all 12 implemented)
├── README.md
└── src/
    ├── main.rs       # CLI subcommands + TUI entry point
    ├── app.rs        # state machine, input handling, file watch, audit log
    ├── config.rs     # config.toml loading, path resolution
    ├── registry.rs   # index.toml CRUD, scan/discover
    ├── syntax.rs     # syntax highlighting (8 file types)
    └── ui.rs         # ratatui rendering (sidebar, viewer, map, scan, history, status)
```

## Design decisions

- **TUI over GUI** — Omarchy is terminal-native. Zero network surface, zero browser.
- **$EDITOR for editing** — no custom text editor. Use the editor you already have.
- **TOML index** — human-readable, git-friendly, easy to edit by hand.
- **References, not storage** — files stay where they are. arioch indexes paths, doesn't move or encrypt them.
- **Append-only audit log** — no database. `grep` and `tail` work on it directly.
# arioch
# arioch
