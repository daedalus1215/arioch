# arioch — Refactor to the Rust Patterns

**Status:** spec / ready to implement in a fresh session
**Pattern reference:** `~/patterns-rust/` (read `design-philosophy.md`, `dependency-hierarchy.md`, `domain/port.md`, `domain/use-case.md` first)
**Type:** pure refactor — **zero behavior change**

---

## 0. Goal & non-goals

**Goal.** Restructure arioch from a flat, coupled single-crate TUI into the *light onion* from `~/patterns-rust/`: a pure, testable `domain/`; all I/O behind **ports**; the `app.rs` + `ui.rs` monoliths split into state-machine / view-models / render. arioch stays a single crate (Option A enforcement from `cross-cutting/crate-graph.md`).

**Non-goals (do NOT do these):**
- No new features, no UI/UX changes, no new CLI flags or env vars.
- No dependency changes (keep `ratatui`, `crossterm`, `toml`, `shellexpand`, `serde_json`, `parking_lot`, `textwrap`, `glob`).
- No workspace-crate split (that is Option B — explicitly deferred).
- No change to the binary name, CLI interface, or on-disk file formats/paths.

**Definition of done:**
- `cargo build` green, `cargo test` green (new domain tests present and passing).
- TUI runs; all modes behave exactly as before (see §5 invariants).
- All 9 CLI commands produce byte-identical output to today.
- The boundary test (§6) passes: `domain/` imports no I/O crate.

---

## 1. Current state (inventory)

8 modules, no `trait`s anywhere, no layering. Violations in bold.

| Module | Lines | Contains | Violation |
| --- | --- | --- | --- |
| `main.rs` | 444 | CLI parse, 9 `cmd_*`, `run_tui`, terminal setup; `set_config_override` (the global); dup `expand_path`, `guess_category` | handlers do I/O (`std::fs` in export/import/init); **duplicated helpers** |
| `app.rs` | 1973 | `Event`, `Mode`, `EditState`, `DialogState`, **42-field `App`**, `impl App` (~40 fns), `edit_*`, `days_to_ymd`, `iso_now`, dup `guess_category`, `expand_path` | **god-object**: input + business + I/O + derived-state in one struct/impl |
| `ui.rs` | ~1440 | `render` + 17 `render_*`, `truncate`, `human_size`, `expand_path_for_check` (3rd copy); reads `app.registry.entries` directly; `std::fs::metadata` for sizes | **render reads the persistence struct**; I/O in render |
| `registry.rs` | ~320 | `Entry`, `Registry{entries,suggestions}`; load/save/parse_toml/to_toml_string (store); `scan_with_config`, `categories`, `entries_in_category`, `is_excluded`, `matches_pattern` (business) | **entity + store + use-cases fused** |
| `knowledge.rs` | 553 | `Danger`, `KnowledgeEntry`, `DetectedKey`, `KnowledgeBase`; `load` (reads user file); `detect`/`detect_ssh`/`detect_keyvalue`/`detect_hosts`/`detect_env` (**core logic**); `builtin_entries` | **data + store + core domain logic fused** |
| `config.rs` | 225 | `CONFIG_OVERRIDE` global (`LazyLock<Mutex>`), `set_config_override`, `active_config_dir`, `Config`, `CategoryColors`, `config_dir`, `editor()` | **process-global mutable state** |
| `annotations.rs` | 65 | `Annotation` (+`covers`), `AnnotationsFile` load/save (TOML) | mostly clean; store logic should move to infra |
| `syntax.rs` | ~400 | `FileType`, `detect_type`, `highlight_*`, `style_*` (ratatui) | pure presentation — stays in the TUI layer |

### I/O call sites that must move behind a port
All in `app.rs` unless noted:
- `std::fs::metadata` — `tick` (index mtime, line 162; entry mtime, 206), `refresh_content` (size check, 1403), `enter_edit` (perm check `mode & 0o200`, 895)
- `std::fs::read_to_string` — `read_history` (278), `handle_suggestions` preview (709), `save_edit` (1079), `refresh_content` (1415)
- `std::fs::write` — `save_edit` (1091)
- `std::fs::OpenOptions` append — `log_action` (262, audit log)
- `std::process::Command` — `open_editor` (1438, `$EDITOR`; **raw-mode dance** around it), `copy_to_clipboard` (1474/1483/1499, `wl-copy`→`xclip`→`xsel`)
- `knowledge.rs` `load` (42) reads user `knowledge.toml`; `registry.rs` load/save; `annotations.rs` load/save; `main.rs` export/import/init use `std::fs`
- `ui.rs` `std::fs::metadata` (324) for file sizes

### Duplicated helpers (consolidate into `domain`)
- `expand_path` — `main.rs:319`, `app.rs:1966`, `ui.rs:1374` (`expand_path_for_check`). → one `domain::rules::expand_path`.
- `guess_category` — `main.rs:324`, `app.rs:1943`. → one `domain::rules::guess_category`.

---

## 2. Target architecture

```
arioch/src/
  main.rs                     # CLI parse + COMPOSITION ROOT: builds stores/adapters,
                              #   threads Config/paths (no more global), dispatches cmd_* / run_tui
  application/
    cli.rs                    # 9 thin cmd_* — build domain params, call use-cases, format stdout
    tui/
      app.rs                  # App: UI state + input handlers; handlers call use-cases/services
                              #   and the ports. No std::fs / std::process directly.
      view.rs                 # view-model structs (sidebar rows, map nodes, dialog, status, help)
      render.rs               # pure: view-models → ratatui widgets (today's ui.rs render_*)
      syntax.rs               # (moved) ratatui highlighting — pure presentation
  domain/                     # NO ratatui, NO crossterm, NO std::fs, NO std::process, NO shellexpand
    mod.rs
    entity.rs                 # Entry
    value.rs                  # Danger, DetectedKey, Annotation, FileMeta, ScanConfig
    ports.rs                  # trait Filesystem, Editor, Clipboard, AuditLog, RegistryStore, AnnotationStore
    rules.rs                  # guess_category, expand_path, is_excluded, matches_pattern,
                              #   visual_order, days_to_ymd, iso_now (pure)
    knowledge.rs              # KnowledgeEntry data + detect/detect_ssh/detect_keyvalue/detect_hosts/detect_env
                              #   (pure — operates on &[KnowledgeEntry]; load is an adapter, not here)
    use_cases/
      mod.rs
      entry.rs                # upsert_entry, remove_entry, tag_entry, set_category, find_entry (pure on &mut Vec<Entry>)
      scan.rs                 # scan_for_suggestions(fs, &ScanConfig) -> Vec<PathBuf>  (uses Filesystem port)
  infra/
    mod.rs
    fs.rs                     # impl Filesystem for RealFs (std::fs)
    process.rs                # impl Editor for ShellEditor; impl Clipboard for Wl/Xclip/Xsel
    index_store.rs            # impl RegistryStore for TomlIndex (save/load + to_toml_string/parse_toml)
    annotations_store.rs      # impl AnnotationStore for TomlAnnotations
    audit_log.rs              # impl AuditLog for FileAuditLog (append + recent)
    config.rs                 # fn load_config(path) -> Config  (reads config.toml; called at the root)
```

**The one hard rule:** `domain/` imports only `std`, `chrono`-free values, and its own items. It is checked by the boundary test (§6). Everything I/O-shaped is a `trait` in `domain/ports.rs`, implemented in `infra/`, bound once in `main.rs` / `tui::run()`.

---

## 3. The ports (the contract)

Signatures are the load-bearing part — implement against exactly these.

```rust
// domain/value.rs
#[derive(Debug, Clone)]
pub struct FileMeta { pub len: u64, pub modified: std::time::SystemTime, pub mode: u32 } // mode = unix perm bits

// domain/ports.rs
pub trait Filesystem: Send + Sync {
    fn read_to_string(&self, path: &std::path::Path) -> std::io::Result<String>;
    fn write(&self, path: &std::path::Path, contents: &str) -> std::io::Result<()>;
    fn metadata(&self, path: &std::path::Path) -> std::io::Result<FileMeta>;
    fn create_dir_all(&self, path: &std::path::Path) -> std::io::Result<()>;
    fn exists(&self, path: &std::path::Path) -> bool;
    fn read_dir(&self, path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>>;
}

pub trait Editor: Send + Sync {
    /// Launch the user's editor on `path`, blocking until exit.
    /// The TUI disables raw mode before and re-enables after — this port only spawns the process.
    fn launch(&self, path: &std::path::Path) -> std::io::Result<()>;
}

pub trait Clipboard: Send + Sync {
    /// Copy `text`; returns true if a backend (wl-copy/xclip/xsel) succeeded.
    fn copy(&self, text: &str) -> bool;
}

pub trait AuditLog: Send + Sync {
    fn append(&self, action: &str, path: &str, details: &str) -> std::io::Result<()>;
    fn recent(&self, n: usize) -> Vec<String>;   // same order/format as today's read_history
}

pub trait RegistryStore: Send + Sync {
    fn load(&self) -> std::io::Result<Vec<entity::Entry>>;
    fn save(&self, entries: &[entity::Entry]) -> std::io::Result<()>;   // TOML, byte-identical to today
}

pub trait AnnotationStore: Send + Sync {
    fn load(&self) -> Vec<value::Annotation>;
    fn save(&self, annotations: &[value::Annotation]) -> std::io::Result<()>;
}
```

- **`Config`** is a domain *value* (not a port). It is loaded once at the composition root via `infra::config::load_config(path)` and threaded through `App` and the CLI. `CategoryColors`, `editor()`, `config_dir()` logic stays but is called at the root, not via a global.
- **Fakes for tests** (per `cross-cutting/testing.md`): `MemFs` (HashMap-backed), `NoopEditor`, `VecClipboard`, `MemAuditLog`, `MemRegistryStore`, `MemAnnotationStore`. Co-locate in `infra/mem.rs` or `#[cfg(test)]`.

---

## 4. Migration plan (each phase ends green: builds + TUI + CLI work)

> Guardrail for every phase: after the change, `cargo build` is clean **and** the TUI still runs and the touched CLI command still behaves. Do not stack red phases.

**Phase 0 — Baseline & characterization tests.**
- Confirm `cargo build`/`cargo check` green; note the current warning count. Smoke-run the TUI and every CLI command; save sample outputs (these are the behavior spec).
- Add characterization tests for the logic that is about to move, *as it exists today* (extract minimally if needed to reach it): `detect_ssh`/`detect_keyvalue`/`detect_hosts`/`detect_env`, `guess_category`, `is_excluded`/`matches_pattern`, `days_to_ymd`, TOML round-trip (`to_toml_string`→`parse_toml`). **Done when:** these tests pass against the current behavior.

**Phase 1 — Extract pure `domain` (no I/O).**
- Create `domain/{mod,entity,value,rules,knowledge}.rs`. Move `Entry`; `Danger`/`DetectedKey`/`Annotation`/`FileMeta`/`ScanConfig`; `guess_category`, `expand_path`, `is_excluded`, `matches_pattern`, `visual_order`, `days_to_ymd`, `iso_now` into `rules`; the `detect_*` fns into `knowledge` operating on `&[KnowledgeEntry]` (drop the file-read from `load` — it becomes an adapter later).
- Point Phase-0 tests at the new `domain` locations. **Done when:** `domain/` compiles with no I/O imports; tests pass.

**Phase 2 — Ports + infra adapters (additive).**
- Write `domain/ports.rs` (§3) and the `infra/` impls, moving the existing `std::fs`/`Command`/TOML code **verbatim** into them. Nothing consumes them yet. **Done when:** compiles; adapters are 1:1 moves of current behavior.

**Phase 3 — Route `App` I/O through ports.**
- Give `App` fields for the ports (`fs`, `editor`, `clipboard`, `audit`, `registry_store`, `annotation_store`) + a `Config`, built in `tui::run()` (composition root). Replace every direct I/O call in `app.rs` with a port call. Preserve the editor raw-mode dance and the `wl→xclip→xsel` fallback order. **Done when:** `app.rs` has zero `std::fs`/`std::process`; TUI identical.

**Phase 4 — Split `Registry`.**
- `Entry` → `domain::entity`. TOML load/save → `infra::index_store` (`RegistryStore`). `scan_with_config` → `domain::use_cases::scan::scan_for_suggestions(fs, &ScanConfig)` (walking via `Filesystem::read_dir`; filtering via `rules`). `add_entry`/`remove_entry`/`tag`/`set_category` → pure use-cases in `domain::use_cases::entry`. `App` holds `entries: Vec<Entry>` + `suggestions: Vec<PathBuf>` and calls `registry_store.save(&entries)` after mutations (mirrors today's mutate-then-save). **Done when:** `registry.rs` is deleted; `cli.rs` + TUI use the use-cases + store port.

**Phase 5 — Kill the global `CONFIG_OVERRIDE`.**
- `main.rs` reads `--config`, resolves the path once, builds `Config` + paths, and passes them into `cmd_*` and `run_tui`. Delete `config::set_config_override`/`active_config_dir`/the `LazyLock<Mutex>`. **Done when:** no `static`/global in `config.rs`; `--config` still works.

**Phase 6 — Split `ui.rs` → `view.rs` + `render.rs`.**
- View-model structs in `view.rs` (built from `App` state / domain projections by converters in `app.rs`). `render.rs` fns take view-models, **not** `&App`/`&Registry`. Move file-size `metadata` reads out of render (compute in the handler, pass the value in). **Done when:** `render.rs` never touches `App` fields or I/O.

**Phase 7 — Partition `impl App`.**
- Keep `Mode`/`EditState`/`DialogState` + UI state in `App`. Input `handle_*` fns become thin: translate a key → call a use-case/service → update state → set a `message`. Consolidate the 3× `expand_path` / 2× `guess_category` onto `domain::rules`. **Done when:** no `handle_*` fn performs I/O or a business mutation inline.

**Phase 8 — Boundary test.**
- Add `tests/boundaries.rs` (crate-graph Option A): walk `src/domain`, assert no `std::fs`, `std::process`, `ratatui`, `crossterm`, `shellexpand`, and no concrete `infra` type name. **Done when:** it passes.

**Phase 9 — Full verification.**
- `cargo build` (0 errors), `cargo test` (all green), `cargo clippy` (no new warnings). Re-run the Phase-0 saved outputs and diff — CLI must be byte-identical. Drive the TUI through every mode. **Done when:** all §5 invariants hold and §6 acceptance passes.

---

## 5. Invariants (behavior must NOT change)

- **CLI:** all 9 commands (`list add remove tag map scan init export import`) emit identical stdout/JSON shapes; exit codes unchanged; `--config` and `--json` behave as before.
- **TUI:** every mode (View, Map, Search, Suggestions, Edit, Annotate, AnnotView, Investigate, Dialog) and every keybinding behaves identically; the editor launch still disables/re-enables raw mode around `$EDITOR`; clipboard still tries `wl-copy`→`xclip`→`xsel`.
- **Files:** `config.toml`, `index.toml`, `annotations.toml`, the audit log, and the user `knowledge.toml` keep identical paths and formats; `index.toml` stays byte-identical on save.
- **No new** env vars, CLI flags, or dependencies.

## 6. Acceptance criteria & verification

- `cargo build` — 0 errors.
- `cargo test` — all pass; Phase-0 characterization tests present and green.
- **Boundary test passes:** `src/domain/**` contains none of `std::fs`, `std::process`, `ratatui`, `crossterm`, `shellexpand`, and no `infra::` type.
- **Structural:** `App` field count reduced (persistence/derived state partitioned); `impl App` contains no `std::fs`/`std::process`; `registry.rs` and the `CONFIG_OVERRIDE` global are gone; exactly one `expand_path` and one `guess_category` remain (in `domain::rules`).
- **Smoke:** TUI runs and all modes work; each CLI command matches the saved Phase-0 output.

## 7. Risks & guardrails

- **`app.rs` (1973 lines) is the biggest risk.** Change one `handle_*`/operation at a time; keep the TUI running after each. Prefer Phase 3 (I/O→ports) before Phase 7 (impl partition) so behavior is stable first.
- **The `detect_*` and scan logic is the product's core** — Phase-0 characterization tests are the safety net; do not "improve" them, only relocate.
- **Preserve exact side-effect ordering** in the editor raw-mode dance and the audit-log append (format + ordering).
- **Do not reach for a workspace split** — single crate + boundary test is the agreed scope (Option A).
- If a phase goes red and resists, revert that phase and split it further rather than carrying red into the next phase.

## 8. Out of scope / follow-ups

- Workspace-crate split (crate-graph Option B) — when a second consumer of `domain` appears or `domain` passes ~6 modules.
- Any feature work, UI redesign, or the matching enoch refactor (separate spec).
