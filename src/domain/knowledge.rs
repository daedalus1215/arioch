//! The knowledge base: what a config key means and how dangerous it is.
//!
//! Pure — operates on `&[KnowledgeEntry]`. Loading/merging the user
//! `knowledge.toml` is an adapter concern (the `knowledge` module today,
//! infra in a later phase), not here.

use super::value::{Danger, DetectedKey};

/// A single knowledge entry for a config key/directive.
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub key: String,
    pub what: String,
    pub why: String,
    pub how: String,
    pub danger: Danger,
}

/// Look up a knowledge entry by key name (case-insensitive).
pub fn lookup<'a>(entries: &'a [KnowledgeEntry], key: &str) -> Option<&'a KnowledgeEntry> {
    let lower = key.to_lowercase();
    entries.iter().find(|e| e.key.to_lowercase() == lower)
}

/// Detect keys in a file's content based on file type.
pub fn detect(entries: &[KnowledgeEntry], content: &str, file_type: &str) -> Vec<DetectedKey> {
    match file_type {
        "ssh" => detect_ssh(entries, content),
        "ini" | "toml" => detect_keyvalue(entries, content),
        "hosts" => detect_hosts(entries, content),
        "env" => detect_env(entries, content),
        _ => detect_keyvalue(entries, content),
    }
}

fn detect_ssh(entries: &[KnowledgeEntry], content: &str) -> Vec<DetectedKey> {
    let mut results = Vec::new();
    let mut section = None;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Host block header
        if trimmed.starts_with("Host ") || trimmed.starts_with("Match ") {
            section = Some(trimmed.to_string());
            continue;
        }

        // Key Value
        if let Some(space_pos) = trimmed.find(' ') {
            let key = &trimmed[..space_pos];
            let value = trimmed[space_pos + 1..].trim().to_string();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            results.push(DetectedKey {
                line: i,
                key: key.to_string(),
                value,
                section: section.clone(),
                entry: lookup(entries, key).cloned(),
            });
        }
    }
    results
}

fn detect_keyvalue(entries: &[KnowledgeEntry], content: &str) -> Vec<DetectedKey> {
    let mut results = Vec::new();
    let mut section = None;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        // Section header: [name]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = Some(trimmed.trim_matches(|c| c == '[' || c == ']').to_string());
            continue;
        }

        // Key = Value or Key: Value or Key Value
        let (key, value) = if let Some(eq_pos) = trimmed.find('=') {
            (trimmed[..eq_pos].trim(), trimmed[eq_pos + 1..].trim())
        } else if let Some(colon_pos) = trimmed.find(':') {
            let k = &trimmed[..colon_pos];
            let v = &trimmed[colon_pos + 1..];
            (k.trim(), v.trim())
        } else {
            continue;
        };

        if key.is_empty() || key.len() > 50 {
            continue;
        }

        results.push(DetectedKey {
            line: i,
            key: key.to_string(),
            value: value.to_string(),
            section: section.clone(),
            entry: lookup(entries, key).cloned(),
        });
    }
    results
}

fn detect_hosts(_entries: &[KnowledgeEntry], content: &str) -> Vec<DetectedKey> {
    let mut results = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let ip = parts[0].to_string();
            let hosts: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
            results.push(DetectedKey {
                line: i,
                key: ip,
                value: hosts.join(", "),
                section: None,
                entry: None,
            });
        }
    }
    results
}

fn detect_env(entries: &[KnowledgeEntry], content: &str) -> Vec<DetectedKey> {
    let mut results = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Remove optional "export " prefix
        let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed);

        if let Some(eq_pos) = stripped.find('=') {
            let key = stripped[..eq_pos].trim();
            let value = stripped[eq_pos + 1..].trim();
            if key.is_empty() {
                continue;
            }
            results.push(DetectedKey {
                line: i,
                key: key.to_string(),
                value: value.to_string(),
                section: None,
                entry: lookup(entries, key).cloned(),
            });
        }
    }
    results
}

/// Built-in knowledge base for common security file keys.
pub fn builtin_entries() -> Vec<KnowledgeEntry> {
    vec![
        // ─── SSH Config ─────────────────────────────────────────────────────
        KnowledgeEntry {
            key: "Host".into(),
            what: "Defines a block of settings that apply to matching hostnames.".into(),
            why: "Scoped config: settings only apply when connecting to these hosts.".into(),
            how: "List patterns separated by spaces. Use * for wildcard, ! for negation.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "HostName".into(),
            what: "The actual hostname or IP to connect to (may differ from the alias).".into(),
            why: "Lets you use short aliases (e.g. 'prod') that resolve to real hostnames.".into(),
            how: "Set to the real DNS name or IP. Must be resolvable by your system.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "User".into(),
            what: "Username to authenticate as when connecting to this host.".into(),
            why: "Overrides the default (your local username) for this host.".into(),
            how: "Set to the remote account name. e.g. 'deploy', 'ec2-user', 'admin'.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "Port".into(),
            what: "TCP port to connect to (default 22).".into(),
            why: "Needed if SSH is running on a non-standard port (common in cloud/containers).".into(),
            how: "Set to the port your SSH daemon listens on. Check with 'ss -tlnp | grep sshd'.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "IdentityFile".into(),
            what: "Path to the private key used for authentication to this host.".into(),
            why: "Lets you use different keys per host without -i flag. Critical for key separation.".into(),
            how: "Relative paths resolve from ~/.ssh/. Use full paths for keys elsewhere.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "ProxyJump".into(),
            what: "Route connection through a bastion/jump host (like ssh -J).".into(),
            why: "Access internal hosts that aren't publicly reachable. Essential for prod environments.".into(),
            how: "Set to the jump host alias or user@host. Can chain multiple: 'bastion1, bastion2'.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "ProxyCommand".into(),
            what: "Custom command to create a tunnel (more flexible than ProxyJump).".into(),
            why: "For non-SSH transports (nc, socat) or complex tunneling scenarios.".into(),
            how: "Usually 'nc %h %p' or 'ssh -W %h:%p bastion'. Use ProxyJump if you can.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "ForwardAgent".into(),
            what: "Forward your SSH agent to the remote host (lets remote use your keys).".into(),
            why: "CONVENIENCE vs SECURITY. Remote hosts can use your local keys while forwarded.".into(),
            how: "Enable per-host only when needed. NEVER enable globally with 'Host *'.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "StrictHostKeyChecking".into(),
            what: "Control whether unknown host keys are auto-accepted (yes/no/accept-new).".into(),
            why: "Disabling = MITM vulnerability. 'accept-new' is a good middle ground.".into(),
            how: "Keep 'yes' or 'accept-new'. Only 'no' for automated provisioning you trust.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "AddKeysToAgent".into(),
            what: "Automatically add keys to your SSH agent after first use.".into(),
            why: "Convenience: avoids re-entering passphrases. Keys persist in memory.".into(),
            how: "'yes' to cache, 'confirm' to ask each time. Fine for most setups.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "IdentitiesOnly".into(),
            what: "Only use the explicitly configured IdentityFile (ignore agent keys).".into(),
            why: "Prevents SSH from trying all your agent keys (slow, may trip rate limits).".into(),
            how: "Set 'yes' when you have many keys in your agent and connect to specific hosts.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "ControlMaster".into(),
            what: "Enable connection multiplexing (reuse one TCP connection for multiple sessions).".into(),
            why: "Faster repeated connections. 'auto' creates socket if not present.".into(),
            how: "Pair with ControlPath and ControlPersist. 'auto' is the safe default.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "ControlPath".into(),
            what: "Socket path for connection multiplexing.".into(),
            why: "Must be unique per host. %C hash avoids long path issues.".into(),
            how: "Use '~/.ssh/cm/%r@%h:%p' or with %C: '~/.ssh/cm/%C'.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "ControlPersist".into(),
            what: "Keep multiplexing socket alive after session ends (timeout duration).".into(),
            why: "Avoids re-handshake for rapid successive connections. 'no' to disable.".into(),
            how: "Set to '60' (seconds) or '5m'. Use 'no' if you want clean disconnects.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "RemoteForward".into(),
            what: "Forward a remote port back to your local machine (reverse tunnel).".into(),
            why: "Access local services (databases, dev servers) from the remote host.".into(),
            how: "Format: [bind_address:]remote_port:local_host:local_port".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "LocalForward".into(),
            what: "Forward a local port to a remote destination (port tunnel).".into(),
            why: "Access remote-only services (databases, APIs) through an encrypted tunnel.".into(),
            how: "Format: [bind_address:]local_port:remote_host:remote_port".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "LogLevel".into(),
            what: "Verbosity of SSH logging (QUIET..DEBUG..DEBUG4).".into(),
            why: "DEBUG helps diagnose auth/routing issues. VERBOSE in prod = noisy.".into(),
            how: "Keep 'INFO' (default). Use 'DEBUG3' temporarily when troubleshooting.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "ServerAliveInterval".into(),
            what: "Send keepalive every N seconds to detect dead connections.".into(),
            why: "Prevents stale connections behind NAT/firewalls. 30-60s is typical.".into(),
            how: "Set to 30-60. Pair with ServerAliveCountMax (default 3).".into(),
            danger: Danger::Safe,
        },

        // ─── AWS Credentials ────────────────────────────────────────────────
        KnowledgeEntry {
            key: "aws_access_key_id".into(),
            what: "Public identifier for your AWS API credentials (AKIA...).".into(),
            why: "NOT secret by itself, but paired with secret key = full account access.".into(),
            how: "Rotate via IAM console if exposed. Never commit to git. Use SSO where possible.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "aws_secret_access_key".into(),
            what: "Private secret that signs AWS API requests. THIS is the actual credential.".into(),
            why: "Full account access if leaked. Treat like a password. Rotate immediately if exposed.".into(),
            how: "Generate via IAM. Store only here or in a secrets manager. Never echo/log it.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "aws_session_token".into(),
            what: "Temporary STS token for time-limited access (MFA/role assumption).".into(),
            why: "Indicates this profile uses temporary credentials (good security practice).".into(),
            how: "Don't set manually — populated by 'aws sso login' or 'aws sts assume-role'.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "region".into(),
            what: "Default AWS region for API calls (e.g. us-east-1, eu-west-2).".into(),
            why: "Wrong region = resource not found errors, or worse: creating resources in wrong place.".into(),
            how: "Set to your primary region. Override per-command with --region flag.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "output".into(),
            what: "CLI output format: json (default), table, or text.".into(),
            why: "table is human-readable; json is for scripting. Doesn't affect security.".into(),
            how: "Set to 'json' for scripting, 'table' for interactive use.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "role_arn".into(),
            what: "ARN of an IAM role to assume for temporary elevated/different access.".into(),
            why: "Lets you switch between accounts/permissions without new static keys.".into(),
            how: "Format: arn:aws:iam::ACCOUNT_ID:role/ROLE_NAME. Set ExternalId if required.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "source_profile".into(),
            what: "Profile to use for authenticating when assuming a role.".into(),
            why: "Chain: source profile authenticates → assumes target role. Avoids hardcoding keys.".into(),
            how: "Set to the name of another [profile] in this file that has valid credentials.".into(),
            danger: Danger::Safe,
        },

        // ─── /etc/hosts ─────────────────────────────────────────────────────
        KnowledgeEntry {
            key: "127.0.0.1".into(),
            what: "Loopback address. Maps names to localhost.".into(),
            why: "Local services bind here. Removing entries can break local tooling.".into(),
            how: "Add 'hostname' entries here for local dev (e.g. 'myapp.localhost').".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "::1".into(),
            what: "IPv6 loopback. Same as 127.0.0.1 but for IPv6.".into(),
            why: "Required for IPv6 localhost resolution. Don't remove.".into(),
            how: "Keep the default 'localhost' mapping. Add IPv6 names alongside IPv4.".into(),
            danger: Danger::Caution,
        },

        // ─── Generic / Common ───────────────────────────────────────────────
        KnowledgeEntry {
            key: "path".into(),
            what: "Filesystem path to a resource (key, config, certificate).".into(),
            why: "Must be absolute or resolvable. Broken path = auth failure or missing config.".into(),
            how: "Use ~ for home. Verify with 'ls -la <path>' after changes.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "timeout".into(),
            what: "Maximum time (seconds) to wait before giving up on an operation.".into(),
            why: "Too low = false failures on slow networks. Too high = hung sessions.".into(),
            how: "30-120s for most operations. Increase for cross-continent connections.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "max_connections".into(),
            what: "Upper limit on concurrent connections to a service.".into(),
            why: "Protects the target from connection exhaustion. Too low = queuing delays.".into(),
            how: "Set based on your workload. Monitor if you hit the limit frequently.".into(),
            danger: Danger::Safe,
        },
        KnowledgeEntry {
            key: "allow".into(),
            what: "Whitelist of permitted values/addresses/users.".into(),
            why: "Security boundary. Removing entries = opening access. Adding = restricting.".into(),
            how: "Add specific entries, not wildcards. Review quarterly for stale entries.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "deny".into(),
            what: "Blacklist of blocked values/addresses/users.".into(),
            why: "Defense-in-depth. Even if allow is broad, deny still blocks.".into(),
            how: "Add known-bad entries. Don't rely solely on deny — pair with allow.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "secret".into(),
            what: "A credential or token used for authentication.".into(),
            why: "NEVER commit, log, or echo. Rotate on any suspected exposure.".into(),
            how: "Generate with 'openssl rand -base64 32' or equivalent. Store in this file only.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "token".into(),
            what: "Bearer token or API key for service authentication.".into(),
            why: "Grants access to the API. Treat with same care as a password.".into(),
            how: "Generate from the service dashboard. Set expiry where possible.".into(),
            danger: Danger::Dangerous,
        },
        KnowledgeEntry {
            key: "certificate".into(),
            what: "Path to or content of a TLS/X.509 certificate.".into(),
            why: "Expires! Check 'openssl x509 -enddate -noout -in cert.pem' regularly.".into(),
            how: "Renew before expiry. Automate with certbot or ACME if public-facing.".into(),
            danger: Danger::Caution,
        },
        KnowledgeEntry {
            key: "private_key".into(),
            what: "The secret half of a key pair. NEVER share or commit.".into(),
            why: "Full compromise if leaked. Anyone with this key impersonates the identity.".into(),
            how: "File perms must be 600. Back up encrypted. Rotate on any suspicion.".into(),
            danger: Danger::Dangerous,
        },
    ]
}

// ─── Characterization tests (Phase 0) ──────────────────────────────────────
// Pin current behavior of the detect_* core logic. Relocated verbatim from
// the old `knowledge` module in Phase 1.

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<KnowledgeEntry> {
        builtin_entries()
    }

    type Row = (
        usize,
        String,
        String,
        Option<String>,
        Option<(String, Danger)>,
    );

    fn rows(dks: Vec<DetectedKey>) -> Vec<Row> {
        dks.into_iter()
            .map(|d| {
                (
                    d.line,
                    d.key,
                    d.value,
                    d.section,
                    d.entry.map(|e| (e.key, e.danger)),
                )
            })
            .collect()
    }

    #[test]
    fn detect_ssh_picks_up_keys_sections_and_skips_comments() {
        let content = "# comment\n\
                       Host prod\n\
                         HostName prod.example.com\n\
                         User   deploy\n\
                       \n\
                       Match host 192.168.*\n\
                         Port 2222\n\
                         ForwardAgent yes\n";
        let got = rows(detect(&entries(), content, "ssh"));
        assert_eq!(
            got,
            vec![
                (
                    2,
                    "HostName".into(),
                    "prod.example.com".into(),
                    Some("Host prod".into()),
                    Some(("HostName".into(), Danger::Safe))
                ),
                (
                    3,
                    "User".into(),
                    "deploy".into(),
                    Some("Host prod".into()),
                    Some(("User".into(), Danger::Safe))
                ),
                (
                    6,
                    "Port".into(),
                    "2222".into(),
                    Some("Match host 192.168.*".into()),
                    Some(("Port".into(), Danger::Safe))
                ),
                (
                    7,
                    "ForwardAgent".into(),
                    "yes".into(),
                    Some("Match host 192.168.*".into()),
                    Some(("ForwardAgent".into(), Danger::Dangerous))
                ),
            ]
        );
    }

    #[test]
    fn detect_ssh_skips_key_only_lines_and_links_unknown_keys() {
        let content = "User\n\
                       IdentityFile ~/.ssh/id_rsa\n\
                       UnknownDirective foo\n";
        let got = rows(detect(&entries(), content, "ssh"));
        assert_eq!(
            got,
            vec![
                (
                    1,
                    "IdentityFile".into(),
                    "~/.ssh/id_rsa".into(),
                    None,
                    Some(("IdentityFile".into(), Danger::Caution))
                ),
                (2, "UnknownDirective".into(), "foo".into(), None, None),
            ]
        );
    }

    #[test]
    fn detect_keyvalue_parses_eq_colon_sections_and_comments() {
        let content = "; ini comment\n\
                       # toml comment\n\
                       [profile default]\n\
                       aws_access_key_id = AKIA123\n\
                       region: us-east-1\n\
                       output json\n";
        let got = rows(detect(&entries(), content, "ini"));
        assert_eq!(
            got,
            vec![
                (
                    3,
                    "aws_access_key_id".into(),
                    "AKIA123".into(),
                    Some("profile default".into()),
                    Some(("aws_access_key_id".into(), Danger::Dangerous))
                ),
                (
                    4,
                    "region".into(),
                    "us-east-1".into(),
                    Some("profile default".into()),
                    Some(("region".into(), Danger::Safe))
                ),
            ]
        );
    }

    #[test]
    fn detect_keyvalue_skips_keys_longer_than_50_chars() {
        let content = format!("{}=x\n{}=y\n", "k".repeat(51), "k".repeat(50));
        let got = rows(detect(&entries(), &content, "toml"));
        assert_eq!(got, vec![(1, "k".repeat(50), "y".into(), None, None)]);
    }

    #[test]
    fn detect_hosts_joins_names_and_never_links_entries() {
        let content = "# comment\n127.0.0.1 localhost myapp.local\n::1 localhost\nsingle\n";
        let got = rows(detect(&entries(), content, "hosts"));
        assert_eq!(
            got,
            vec![
                (
                    1,
                    "127.0.0.1".into(),
                    "localhost, myapp.local".into(),
                    None,
                    None
                ),
                (2, "::1".into(), "localhost".into(), None, None),
            ]
        );
    }

    #[test]
    fn detect_env_strips_export_prefix_and_lookup_is_case_insensitive() {
        let content = "# comment\nexport PATH=/usr/bin\nTOKEN=abc\nno-equals-here\n";
        let got = rows(detect(&entries(), content, "env"));
        assert_eq!(
            got,
            vec![
                (
                    1,
                    "PATH".into(),
                    "/usr/bin".into(),
                    None,
                    Some(("path".into(), Danger::Safe))
                ),
                (
                    2,
                    "TOKEN".into(),
                    "abc".into(),
                    None,
                    Some(("token".into(), Danger::Dangerous))
                ),
            ]
        );
    }

    #[test]
    fn detect_dispatches_on_file_type_and_defaults_to_keyvalue() {
        let kv = "a = b\n";
        let ssh = "Host h\n  Port 22\n";
        assert_eq!(
            rows(detect(&entries(), kv, "ini")),
            rows(detect(&entries(), kv, "toml"))
        );
        assert_eq!(
            rows(detect(&entries(), kv, "toml")),
            rows(detect(&entries(), kv, "what-is-this"))
        );
        assert_ne!(
            rows(detect(&entries(), ssh, "ssh")),
            rows(detect(&entries(), ssh, "toml"))
        );
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let es = entries();
        assert!(lookup(&es, "STRICTHOSTKEYCHECKING").is_some());
        assert_eq!(lookup(&es, "identityfile").unwrap().danger, Danger::Caution);
        assert!(lookup(&es, "not_a_key").is_none());
    }
}
