//! File-backed audit log (`<config-dir>/arioch/history.log`).

use std::io::Write;
use std::path::Path;

use crate::domain::ports::AuditLog;

pub struct FileAuditLog {
    log_path: std::path::PathBuf,
}

impl FileAuditLog {
    pub fn new(config_dir: &Path) -> Self {
        let mut p = config_dir.to_path_buf();
        p.push("arioch");
        p.push("history.log");
        Self { log_path: p }
    }
}

impl AuditLog for FileAuditLog {
    fn append(&self, action: &str, path: &str, details: &str) -> std::io::Result<()> {
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
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    fn recent(&self, n: usize) -> Vec<String> {
        match std::fs::read_to_string(&self.log_path) {
            Ok(content) => content
                .lines()
                .rev()
                .take(n)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| s.to_string())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}
