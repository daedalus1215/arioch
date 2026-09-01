use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub scan_paths: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub scan_patterns: Vec<String>,
    pub scan_depth: usize,
    pub max_file_size: usize,
    pub refresh_interval: u64,
    pub editor: Option<String>,
    pub colors: CategoryColors,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CategoryColors {
    pub ssh_keys: Option<String>,
    pub cloud_creds: Option<String>,
    pub system_config: Option<String>,
    pub certificates: Option<String>,
    pub gpg: Option<String>,
    pub secrets: Option<String>,
    pub app_config: Option<String>,
    pub other: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan_paths: vec![
                "~/.ssh".into(),
                "~/.config".into(),
                "/etc/ssh".into(),
                "/etc/ssl".into(),
                "~/.gnupg".into(),
            ],
            exclude_paths: vec![
                "/etc/ssl/certs".into(),
                "/etc/ssl/certs.d".into(),
                "~/.config/BraveSoftware".into(),
                "~/.config/google-chrome".into(),
                "~/.config/chromium".into(),
                "~/.config/firefox".into(),
                "~/.config/Code".into(),
                "~/.config/VSCode".into(),
                "~/.config/git".into(),
                "~/.config/gh".into(),
            ],
            scan_patterns: vec![
                "*.pub".into(),
                "id_*".into(),
                "known_hosts".into(),
                "config".into(),
                "ssh_config".into(),
                "authorized_keys".into(),
                "*.pem".into(),
                "*.crt".into(),
                "*.key".into(),
                "*.p12".into(),
                "*.pfx".into(),
                "*.cfg".into(),
                "*.conf".into(),
                "*.toml".into(),
                "*.yaml".into(),
                "*.yml".into(),
                "*.json".into(),
                "hosts".into(),
                "shadow".into(),
                "passwd".into(),
                "sudoers".into(),
                "*.env".into(),
                ".env*".into(),
                "*.secret".into(),
                "*.credentials".into(),
            ],
            scan_depth: 5,
            max_file_size: 1_048_576,
            refresh_interval: 2,
            editor: None,
            colors: CategoryColors::default(),
        }
    }
}

impl Default for CategoryColors {
    fn default() -> Self {
        Self {
            ssh_keys: None,
            cloud_creds: None,
            system_config: None,
            certificates: None,
            gpg: None,
            secrets: None,
            app_config: None,
            other: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| format!("{}/.config", h).into()))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }

    fn config_path() -> PathBuf {
        let mut p = Self::config_dir();
        p.push("arioch");
        p.push("config.toml");
        p
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }
        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("arioch: warning: invalid config.toml ({}), using defaults", e);
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("arioch: warning: cannot read config.toml ({}), using defaults", e);
                Self::default()
            }
        }
    }

    pub fn editor(&self) -> String {
        let editor = self
            .editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "nano".to_string());
        // Guard against no-op editors (common in scripts: EDITOR=true)
        match editor.as_str() {
            "true" | "false" | ":" | "" => "nano".to_string(),
            _ => editor,
        }
    }

    /// Resolve a category name to a terminal color. Uses config overrides if set.
    pub fn category_color(&self, category: &str) -> ratatui::style::Color {
        // Check for custom override
        let custom = match category {
            "ssh-keys" | "ssh" => &self.colors.ssh_keys,
            "cloud-creds" | "credentials" | "creds" => &self.colors.cloud_creds,
            "os-config" | "config" | "configs" | "system" => &self.colors.system_config,
            "certs" | "certificates" | "ssl" => &self.colors.certificates,
            "gpg" | "keys" => &self.colors.gpg,
            "secrets" | "env" => &self.colors.secrets,
            "app-config" | "app" => &self.colors.app_config,
            _ => &self.colors.other,
        };

        if let Some(ref color_str) = custom {
            return parse_color(color_str);
        }

        // Defaults
        match category {
            "ssh-keys" | "ssh" => ratatui::style::Color::Green,
            "cloud-creds" | "credentials" | "creds" => ratatui::style::Color::Blue,
            "os-config" | "config" | "configs" | "system" => ratatui::style::Color::Yellow,
            "certs" | "certificates" | "ssl" => ratatui::style::Color::Red,
            "gpg" | "keys" => ratatui::style::Color::Magenta,
            "secrets" | "env" => ratatui::style::Color::Cyan,
            "app-config" | "app" => ratatui::style::Color::Green,
            _ => ratatui::style::Color::Gray,
        }
    }
}

fn parse_color(s: &str) -> ratatui::style::Color {
    match s.to_lowercase().as_str() {
        "red" => ratatui::style::Color::Red,
        "green" => ratatui::style::Color::Green,
        "yellow" => ratatui::style::Color::Yellow,
        "blue" => ratatui::style::Color::Blue,
        "magenta" => ratatui::style::Color::Magenta,
        "cyan" => ratatui::style::Color::Cyan,
        "gray" | "grey" => ratatui::style::Color::Gray,
        "darkgray" | "darkgrey" => ratatui::style::Color::DarkGray,
        "white" => ratatui::style::Color::White,
        "black" => ratatui::style::Color::Black,
        // Hex colors like "#ff0000"
        _ if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
            ratatui::style::Color::Rgb(r, g, b)
        }
        _ => ratatui::style::Color::Gray,
    }
}
