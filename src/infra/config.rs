//! Config file loading (the `Config` type itself stays in `src/config.rs`
//! until its ratatui color logic is sorted in a later phase).

use crate::config::Config;

/// Load `config.toml` from `path`. Missing file → defaults; unreadable or
/// unparseable → defaults plus the exact warning lines of today's load.
pub fn load_config(path: &std::path::Path) -> Config {
    if !path.exists() {
        return Config::default();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<Config>(&content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("arioch: warning: invalid config.toml ({}), using defaults", e);
                Config::default()
            }
        },
        Err(e) => {
            eprintln!("arioch: warning: cannot read config.toml ({}), using defaults", e);
            Config::default()
        }
    }
}
