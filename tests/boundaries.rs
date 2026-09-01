//! Boundary test (SPECS.md §4 Phase 8, crate-graph Option A).
//!
//! arioch is a single crate, so the dependency rule "`domain/` must not
//! touch I/O, UI, or infrastructure" is enforced here by walking the
//! source of `src/domain/**` and asserting that no banned token appears
//! in code. Comments and string/char literals are stripped first, so the
//! domain's own documentation of this rule does not trip the test.

use std::path::{Path, PathBuf};

/// Tokens that must not appear in domain code. `infra::` covers every
/// concrete infrastructure type by path; the explicit type names guard
/// against name-only imports.
const BANNED: &[&str] = &[
    "std::fs",
    "std::process",
    "ratatui",
    "crossterm",
    "shellexpand",
    "infra::",
    "TomlIndex",
    "RealFs",
    "ShellEditor",
    "FileAuditLog",
    "TomlAnnotations",
];

/// Recursively collect every `.rs` file under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Remove comments and string/char literals, leaving code only.
///
/// Handles line comments, (nested) block comments, `"..."` strings with
/// backslash escapes, `'c'`/`'\n'` char literals, and disambiguates
/// lifetimes (`'a`) from char literals.
fn strip_comments_and_strings(src: &str) -> String {
    #[derive(PartialEq)]
    enum State {
        Normal,
        Line,
        Block,
        Str,
        Char,
    }

    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut state = State::Normal;
    let mut block_depth = 0usize;

    while let Some(c) = chars.next() {
        match state {
            State::Normal => match c {
                '/' if matches!(chars.peek(), Some(&'/')) => {
                    chars.next();
                    state = State::Line;
                }
                '/' if matches!(chars.peek(), Some(&'*')) => {
                    chars.next();
                    block_depth = 1;
                    state = State::Block;
                }
                '"' => state = State::Str,
                '\'' => {
                    // Char literal: `'c'` or `'\x'`; lifetime: `'ident`.
                    let is_char_literal = match chars.peek() {
                        Some(&'\\') => true,
                        Some(&next) => {
                            let after = chars.clone().nth(1);
                            next != '\'' && after == Some('\'')
                        }
                        None => false,
                    };
                    if is_char_literal {
                        state = State::Char;
                    } else {
                        out.push(c);
                    }
                }
                _ => out.push(c),
            },
            State::Line => {
                if c == '\n' {
                    state = State::Normal;
                    out.push(c);
                }
            }
            State::Block => {
                if c == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    block_depth += 1;
                } else if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    block_depth -= 1;
                    if block_depth == 0 {
                        state = State::Normal;
                    }
                }
            }
            State::Str => {
                if c == '\\' {
                    chars.next();
                } else if c == '"' {
                    state = State::Normal;
                }
            }
            State::Char => {
                if c == '\\' {
                    chars.next();
                } else if c == '\'' {
                    state = State::Normal;
                }
            }
        }
    }
    out
}

#[test]
fn domain_stays_pure_no_io_ui_or_infra() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain");
    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);
    assert!(
        !files.is_empty(),
        "boundary test found no .rs files under src/domain — is the layout right?"
    );

    for file in &files {
        let src = std::fs::read_to_string(file).unwrap();
        let code = strip_comments_and_strings(&src);
        for banned in BANNED {
            assert!(
                !code.contains(banned),
                "{} uses banned token `{}` — domain/ must stay pure (SPECS.md §2)",
                file.display(),
                banned
            );
        }
    }
}
