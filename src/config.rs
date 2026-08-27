use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Global override for config directory (set via --config flag).
static CONFIG_OVERRIDE: std::sync::LazyLock<parking_lot::Mutex<Option<PathBuf>>> =
    std::sync::LazyLock::new(|| parking_lot::Mutex::new(None));

/// Set a custom config directory. All config/index paths will be resolved under this.
pub fn set_config_override(path: PathBuf) {
    let mut guard = CONFIG_OVERRIDE.lock();
    *guard = Some(path);
}

/// Get the active config directory (override or default).
pub fn active_config_dir() -> PathBuf {
    let guard = CONFIG_OVERRIDE.lock();
    if let Some(ref p) = *guard {
        p.clone()
    } else {
        Config::config_dir()
    }
}
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
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "nano".to_string())
    }
}
