//! Pure domain rules: category guessing, path expansion, scan filtering,
//! ordering, and date math.
use super::entity::Entry;
use glob::Pattern;
use std::path::Path;

/// Guess the category of a path — TUI variant (used by the TUI add flows).
///
/// NOTE: differs from `guess_category_cli` on purpose. The two copies existed
/// before the refactor with different rules and category names; the
/// zero-behavior-change contract (SPECS.md §5) keeps both.
pub fn guess_category_tui(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains(".ssh") || lower.contains("ssh_key") || lower.contains("id_") {
        return "ssh-keys".into();
    }
    if lower.contains(".pem") || lower.contains(".crt") || lower.contains(".cert") {
        return "certificates".into();
    }
    if lower.contains(".gnupg") || lower.contains("gpg") {
        return "gpg".into();
    }
    if lower.contains("credentials") || lower.contains("secret") || lower.contains(".env") {
        return "secrets".into();
    }
    if lower.contains("/etc/") {
        return "system-config".into();
    }
    if lower.contains(".config") {
        return "app-config".into();
    }
    "other".into()
}

/// Guess the category of a path — CLI variant (used by `arioch add`).
///
/// NOTE: differs from `guess_category_tui` on purpose (see its note).
pub fn guess_category_cli(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains(".ssh") || lower.contains("id_") {
        "ssh-keys".to_string()
    } else if lower.contains("cert") || lower.contains(".pem") || lower.contains(".crt") {
        "certs".to_string()
    } else if lower.contains("credential") || lower.contains("token") || lower.contains("secret") {
        "creds".to_string()
    } else if lower.contains("config")
        || lower.contains(".toml")
        || lower.contains(".yaml")
        || lower.contains(".yml")
    {
        "configs".to_string()
    } else {
        "other".to_string()
    }
}

/// Expand a leading `~` to `$HOME`.
///
/// NOTE (preserved quirk): `~user` is NOT treated specially — `user` ends up
/// as a path segment under `$HOME`. The CLI layer keeps its own shellexpand
/// wrapper (shellexpand is banned in `domain/`), which does handle `~user`.
pub fn expand_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}

/// True if `path` is under any exclude prefix.
///
/// NOTE (preserved quirk): bare string `starts_with`, not path-aware —
/// excluding `/etc/ssl` also excludes `/etc/ssl-evil`.
pub fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    excludes.iter().any(|ex| path_str.starts_with(ex.as_str()))
}

/// True if the path's file name matches any glob pattern (invalid patterns ignored).
pub fn matches_pattern(path: &Path, patterns: &[String]) -> bool {
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        for pattern_str in patterns {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches(filename) {
                    return true;
                }
            }
        }
    }
    false
}

/// Sorted, deduped, non-empty categories of `entries`.
pub fn categories(entries: &[Entry]) -> Vec<String> {
    let mut cats: Vec<String> = entries
        .iter()
        .map(|e| e.category.clone())
        .filter(|c| !c.is_empty())
        .collect();
    cats.sort();
    cats.dedup();
    cats
}

/// Indices of entries in `category`.
pub fn entries_in_category(entries: &[Entry], category: &str) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.category == category)
        .map(|(i, _)| i)
        .collect()
}

/// Entry indices in visual display order (grouped by category, alphabetical).
pub fn visual_order(entries: &[Entry]) -> Vec<usize> {
    let mut order = Vec::new();
    for category in categories(entries) {
        order.extend(entries_in_category(entries, &category));
    }
    order
}

/// Convert days since UNIX epoch to (year, month, day).
/// Civil-from-days algorithm (Howard Hinnant).
pub fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as i64, d as i64)
}

/// ISO-8601 UTC timestamp for annotation records.
pub fn iso_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs() as i64;
            let (year, month, day) = days_to_ymd(secs / 86400);
            format!(
                "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                year,
                month,
                day,
                (secs % 86400) / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

// ─── Characterization tests (Phase 0) ──────────────────────────────────────
// Pin current behavior of the pure helpers. Relocated from app.rs/main.rs in
// Phase 1; the two guess_category variants and their divergence are pinned
// separately and must both keep passing.

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry {
            path: path.into(),
            category: String::new(),
            tags: Vec::new(),
            description: String::new(),
            alias: None,
            related: Vec::new(),
        }
    }

    /// Inverse of `days_to_ymd` (Hinnant days_from_civil), for round-trip checks.
    fn ymd_to_days(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 };
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    #[test]
    fn days_to_ymd_known_dates() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        assert_eq!(days_to_ymd(1), (1970, 1, 2));
        assert_eq!(days_to_ymd(-1), (1969, 12, 31));
        assert_eq!(days_to_ymd(59), (1970, 3, 1));
        assert_eq!(days_to_ymd(18321), (2020, 2, 29)); // leap year
        assert_eq!(days_to_ymd(11016), (2000, 2, 29)); // century leap year
        assert_eq!(days_to_ymd(-25567), (1900, 1, 1)); // non-leap century boundary
        assert_eq!(days_to_ymd(20088), (2024, 12, 31));
    }

    #[test]
    fn days_to_ymd_round_trips_over_wide_range() {
        for days in -100_000..=100_000 {
            let (y, m, d) = days_to_ymd(days);
            assert_eq!(ymd_to_days(y, m, d), days, "round trip failed at {days}");
        }
    }

    #[test]
    fn iso_now_has_utc_shape_and_todays_date() {
        let now = iso_now();
        assert_eq!(now.len(), 20, "unexpected shape: {now}");
        assert!(now.ends_with('Z'));
        assert_eq!(&now[4..5], "-");
        assert_eq!(&now[7..8], "-");
        assert_eq!(&now[10..11], "T");
        assert_eq!(&now[13..14], ":");
        assert_eq!(&now[16..17], ":");
        let y: i64 = now[0..4].parse().unwrap();
        let m: u32 = now[5..7].parse().unwrap();
        let d: u32 = now[8..10].parse().unwrap();
        let h: u32 = now[11..13].parse().unwrap();
        let min: u32 = now[14..16].parse().unwrap();
        let s: u32 = now[17..19].parse().unwrap();
        assert!(y >= 1970 && y <= 2100);
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
        assert!(h < 24 && min < 60 && s < 60);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        // allow today or yesterday (midnight race)
        let today = days_to_ymd(secs / 86400);
        let yesterday = days_to_ymd(secs / 86400 - 1);
        assert!(
            (y, m as i64, d as i64) == today || (y, m as i64, d as i64) == yesterday,
            "date part {y:04}-{m:02}-{d:02} is not today"
        );
    }

    #[test]
    fn tui_guess_category_branches_and_precedence() {
        assert_eq!(guess_category_tui("/home/u/.ssh/config"), "ssh-keys");
        assert_eq!(guess_category_tui("/home/u/ssh_key.pem"), "ssh-keys"); // ssh_key beats .pem
        assert_eq!(guess_category_tui("/home/u/keys/id_rsa"), "ssh-keys");
        assert_eq!(guess_category_tui("/etc/ssl/server.pem"), "certificates");
        assert_eq!(guess_category_tui("/etc/ssl/x.crt"), "certificates");
        assert_eq!(guess_category_tui("/etc/ssl/my.cert"), "certificates"); // .cert extension
        assert_eq!(
            guess_category_tui("/etc/ssl/cert-bundle"),
            "system-config"
        ); // "cert" without a leading dot does NOT match ".cert"
        assert_eq!(guess_category_tui("/home/u/.gnupg/secring.gpg"), "gpg");
        assert_eq!(guess_category_tui("/home/u/.aws/credentials"), "secrets");
        assert_eq!(guess_category_tui("/home/u/.env"), "secrets");
        assert_eq!(guess_category_tui("/home/u/secret.txt"), "secrets");
        assert_eq!(guess_category_tui("/etc/hosts"), "system-config");
        assert_eq!(
            guess_category_tui("/home/u/.config/app/settings.json"),
            "app-config"
        );
        assert_eq!(guess_category_tui("/var/log/x"), "other");
        // case-insensitive
        assert_eq!(guess_category_tui("/HOME/U/.SSH/CONFIG"), "ssh-keys");
    }

    #[test]
    fn cli_guess_category_branches() {
        assert_eq!(guess_category_cli("/home/u/.ssh/config"), "ssh-keys"); // .ssh
        assert_eq!(guess_category_cli("/home/u/id_rsa"), "ssh-keys"); // id_
        assert_eq!(guess_category_cli("/etc/ssl/server.pem"), "certs"); // .pem
        assert_eq!(guess_category_cli("/etc/ssl/certificate"), "certs"); // cert substring
        assert_eq!(guess_category_cli("/etc/ssl/x.crt"), "certs"); // .crt
        assert_eq!(guess_category_cli("/home/u/.aws/credentials"), "creds"); // credential substring
        assert_eq!(guess_category_cli("/home/u/tokens.json"), "creds"); // token
        assert_eq!(guess_category_cli("/home/u/secret.txt"), "creds"); // secret
        assert_eq!(guess_category_cli("/home/u/app.toml"), "configs"); // .toml
        assert_eq!(guess_category_cli("/home/u/.config/x"), "configs"); // config substring
        assert_eq!(guess_category_cli("/home/u/cfg.yaml"), "configs"); // .yaml
        assert_eq!(guess_category_cli("/home/u/cfg.yml"), "configs"); // .yml
        assert_eq!(guess_category_cli("/home/u/.env"), "other"); // no .env branch in the CLI copy
        assert_eq!(guess_category_cli("/home/u/keys.gpg"), "other"); // no gpg branch in the CLI copy
        assert_eq!(guess_category_cli("/var/log/x"), "other");
        // case-insensitive
        assert_eq!(guess_category_cli("/HOME/U/.SSH/CONFIG"), "ssh-keys");
    }

    #[test]
    fn the_two_guess_category_variants_diverge() {
        // Pinned divergence: same input, different categories by design.
        assert_ne!(
            guess_category_tui("/etc/ssl/server.pem"),
            guess_category_cli("/etc/ssl/server.pem")
        );
        assert_ne!(
            guess_category_tui("/home/u/.aws/credentials"),
            guess_category_cli("/home/u/.aws/credentials")
        );
        assert_ne!(guess_category_tui("/home/u/.env"), guess_category_cli("/home/u/.env"));
    }

    #[test]
    fn expand_path_tilde_uses_home() {
        match std::env::var("HOME") {
            Ok(home) => {
                assert_eq!(expand_path("~/x"), format!("{home}/x"));
                assert_eq!(expand_path("~"), home);
                // quirk: "~user" is not treated specially — "user" ends up under $HOME
                assert_eq!(expand_path("~user/x"), format!("{home}user/x"));
            }
            Err(_) => {
                assert_eq!(expand_path("~/x"), "~/x");
            }
        }
        assert_eq!(expand_path("/abs/path"), "/abs/path");
        assert_eq!(expand_path("rel/path"), "rel/path");
    }

    #[test]
    fn is_excluded_is_bare_string_prefix_match() {
        let excludes = vec!["/etc/ssl".to_string(), "/home/u/.config/git".to_string()];
        assert!(is_excluded(Path::new("/etc/ssl/certs/x"), &excludes));
        assert!(!is_excluded(Path::new("/etc/ssh/sshd_config"), &excludes));
        // quirk: plain starts_with, not path-aware — "/etc/ssl" also excludes "/etc/ssl-evil"
        assert!(is_excluded(Path::new("/etc/ssl-evil/x"), &excludes));
    }

    #[test]
    fn matches_pattern_matches_filename_only_and_ignores_invalid_patterns() {
        let patterns = vec![
            "id_*".to_string(),
            "*.pem".to_string(),
            "config".to_string(),
            "[".to_string(),
        ];
        assert!(matches_pattern(Path::new("/a/b/id_rsa"), &patterns));
        assert!(matches_pattern(Path::new("/a/b/cert.pem"), &patterns));
        assert!(matches_pattern(Path::new("/etc/ssh/config"), &patterns));
        assert!(!matches_pattern(Path::new("/a/b/other.txt"), &patterns));
        assert!(!matches_pattern(Path::new("/"), &patterns));
    }

    #[test]
    fn categories_are_sorted_deduped_and_skip_empty() {
        let mut e1 = entry("/a");
        e1.category = "b".into();
        let mut e2 = entry("/b");
        e2.category = "a".into();
        let mut e3 = entry("/c");
        e3.category = "b".into();
        let es = vec![e1, e2, e3, entry("/d")];
        assert_eq!(categories(&es), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(entries_in_category(&es, "b"), vec![0usize, 2]);
    }

    #[test]
    fn visual_order_groups_by_category_alphabetical_then_index() {
        let mut a = entry("/z/a");
        a.category = "zeta".into();
        let mut b = entry("/a/b");
        b.category = "alpha".into();
        let mut c = entry("/m/c");
        c.category = "zeta".into();
        let mut d = entry("/x/d");
        d.category = "alpha".into();
        let es = vec![a, b, c, d];
        // alpha entries (indices 1,3) before zeta entries (indices 0,2), index order within
        assert_eq!(visual_order(&es), vec![1, 3, 0, 2]);
    }
}
