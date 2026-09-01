//! Process adapters: the user's editor and the system clipboard.

use std::path::Path;

use crate::domain::ports::{Clipboard, Editor};

/// Spawns a fixed editor binary (the resolved `Config::editor()` value).
/// Raw-mode toggling around the spawn stays in the TUI, not here.
pub struct ShellEditor {
    editor: String,
}

impl ShellEditor {
    pub fn new(editor: String) -> Self {
        Self { editor }
    }
}

impl Editor for ShellEditor {
    fn launch(&self, path: &Path) -> std::io::Result<()> {
        std::process::Command::new(&self.editor).arg(path).status().map(|_| ())
    }
}

/// Clipboard backends in the preserved fallback order: wl-copy → xclip → xsel.
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn copy(&self, text: &str) -> bool {
        // Try wl-copy (Wayland)
        if std::process::Command::new("wl-copy")
            .arg(text)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }
        // Try xclip (X11)
        if let Ok(mut child) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if child.wait().map(|s| s.success()).unwrap_or(false) {
                        return true;
                    }
                }
            }
        }
        // Try xsel (X11 fallback)
        if let Ok(mut child) = std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if child.wait().map(|s| s.success()).unwrap_or(false) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
