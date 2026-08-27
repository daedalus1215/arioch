use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Toml,
    Json,
    Yaml,
    Ini,
    Ssh,
    Env,
    Pem,
    Shell,
    Plain,
}

pub fn detect_type(path: &str, first_line: &str) -> FileType {
    let lower = path.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");

    // Extension-based detection
    match ext {
        "toml" | "cfg" => return FileType::Toml,
        "json" => return FileType::Json,
        "yaml" | "yml" => return FileType::Yaml,
        "conf" | "ini" => return FileType::Ini,
        "pem" | "crt" | "key" | "p12" | "pfx" => return FileType::Pem,
        "sh" | "bash" => return FileType::Shell,
        "env" => return FileType::Env,
        _ => {}
    }

    // Filename-based detection
    let filename = lower.rsplit('/').next().unwrap_or(&lower);
    if filename == "config" || filename == "ssh_config" {
        if lower.contains(".ssh") || lower.contains("/etc/ssh") {
            return FileType::Ssh;
        }
        return FileType::Ini;
    }
    if filename == "known_hosts" || filename == "authorized_keys" {
        return FileType::Ssh;
    }
    if filename == "hosts" || filename == "shadow" || filename == "sudoers" || filename == "passwd" {
        return FileType::Ini;
    }
    if filename.starts_with(".env") {
        return FileType::Env;
    }
    if filename.starts_with("id_") {
        return FileType::Ssh;
    }

    // Content-based detection
    if first_line.starts_with("-----BEGIN") {
        return FileType::Pem;
    }

    FileType::Plain
}

pub fn highlight_line(line: &str, file_type: FileType) -> Line<'_> {
    match file_type {
        FileType::Toml => highlight_toml(line),
        FileType::Json => highlight_json(line),
        FileType::Yaml => highlight_yaml(line),
        FileType::Ini => highlight_ini(line),
        FileType::Ssh => highlight_ssh(line),
        FileType::Env => highlight_env(line),
        FileType::Pem => highlight_pem(line),
        FileType::Shell => highlight_shell(line),
        FileType::Plain => Line::from(line.to_string()),
    }
}

fn highlight_toml(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    if let Some(eq_pos) = trimmed.find('=') {
        let key = &trimmed[..eq_pos];
        let rest = &trimmed[eq_pos..];
        spans.push(Span::styled(
            key.to_string(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw("=".to_string()));
        let value = rest[1..].trim();
        spans.push(Span::raw(" ".to_string()));
        spans.push(Span::styled(
            value.to_string(),
            style_toml_value(value),
        ));
    } else if trimmed.starts_with('[') {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw(trimmed.to_string()));
    }

    Line::from(spans)
}

fn style_toml_value(value: &str) -> Style {
    let v = value.trim();
    if v.starts_with('"') || v.starts_with('\'') {
        Style::default().fg(Color::Green)
    } else if v == "true" || v == "false" {
        Style::default().fg(Color::Yellow)
    } else if v.parse::<f64>().is_ok() {
        Style::default().fg(Color::Magenta)
    } else if v.starts_with('[') || v.starts_with('{') {
        Style::default().fg(Color::Reset)
    } else {
        Style::default().fg(Color::Reset)
    }
}

fn highlight_json(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return Line::from(line.to_string());
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    // Key: "value" pattern
    if let Some(colon_pos) = trimmed.find(':') {
        let key = &trimmed[..colon_pos];
        let rest = &trimmed[colon_pos + 1..];

        if key.trim().starts_with('"') {
            spans.push(Span::styled(
                key.to_string(),
                Style::default().fg(Color::Cyan),
            ));
        } else {
            spans.push(Span::raw(key.to_string()));
        }
        spans.push(Span::raw(":".to_string()));

        let value = rest.trim();
        spans.push(Span::raw(" ".to_string()));
        spans.push(Span::styled(
            value.to_string(),
            style_json_value(value),
        ));
    } else {
        spans.push(Span::raw(trimmed.to_string()));
    }

    Line::from(spans)
}

fn style_json_value(value: &str) -> Style {
    let v = value.trim().trim_end_matches(',').trim();
    if v.starts_with('"') {
        Style::default().fg(Color::Green)
    } else if v == "true" || v == "false" || v == "null" {
        Style::default().fg(Color::Yellow)
    } else if v.parse::<f64>().is_ok() {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::Reset)
    }
}

fn highlight_yaml(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    if let Some(colon_pos) = trimmed.find(':') {
        let key = &trimmed[..colon_pos];
        let rest = &trimmed[colon_pos..];
        spans.push(Span::styled(
            key.to_string(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw(": ".to_string()));
        let value = rest[2..].trim();
        spans.push(Span::styled(
            value.to_string(),
            style_yaml_value(value),
        ));
    } else if trimmed.starts_with("- ") {
        spans.push(Span::styled(
            "- ".to_string(),
            Style::default().fg(Color::Magenta),
        ));
        spans.push(Span::raw(trimmed[2..].to_string()));
    } else if trimmed.starts_with('&') || trimmed.starts_with('*') {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default().fg(Color::Magenta),
        ));
    } else {
        spans.push(Span::raw(trimmed.to_string()));
    }

    Line::from(spans)
}

fn style_yaml_value(value: &str) -> Style {
    let v = value.trim();
    if v.starts_with('"') || v.starts_with('\'') {
        Style::default().fg(Color::Green)
    } else if v == "true" || v == "false" || v == "null" || v == "~" {
        Style::default().fg(Color::Yellow)
    } else if v.parse::<f64>().is_ok() {
        Style::default().fg(Color::Magenta)
    } else if v.is_empty() {
        Style::default().fg(Color::Reset)
    } else {
        Style::default().fg(Color::Green)
    }
}

fn highlight_ini(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        spans.push(Span::styled(
            trimmed.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(eq_pos) = trimmed.find('=') {
        let key = &trimmed[..eq_pos];
        let rest = &trimmed[eq_pos + 1..];
        spans.push(Span::styled(
            key.to_string(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw("=".to_string()));
        spans.push(Span::raw(rest.to_string()));
    } else {
        spans.push(Span::raw(trimmed.to_string()));
    }

    Line::from(spans)
}

fn highlight_ssh(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    // SSH config: Host/Host patterns
    let ssh_directives = [
        "Host ", "Host*", "User ", "Port ", "IdentityFile ", "ProxyJump ",
        "ForwardAgent ", "StrictHostKeyChecking ", "AddKeysToAgent ",
        "IdentitiesOnly ", "Compression ", "ConnectTimeout ",
    ];

    for directive in &ssh_directives {
        if trimmed.starts_with(directive) {
            let key = &trimmed[..directive.len() - 1];
            let rest = &trimmed[directive.len() - 1..];
            spans.push(Span::styled(
                key.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" ".to_string()));
            spans.push(Span::raw(rest.trim_start().to_string()));
            return Line::from(spans);
        }
    }

    spans.push(Span::raw(trimmed.to_string()));
    Line::from(spans)
}

fn highlight_env(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    if trimmed.starts_with("export ") {
        spans.push(Span::styled(
            "export ".to_string(),
            Style::default().fg(Color::Magenta),
        ));
        let rest = &trimmed[7..];
        if let Some(eq_pos) = rest.find('=') {
            spans.push(Span::styled(
                rest[..eq_pos].to_string(),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::raw("=".to_string()));
            spans.push(Span::styled(
                rest[eq_pos + 1..].to_string(),
                Style::default().fg(Color::Green),
            ));
        } else {
            spans.push(Span::raw(rest.to_string()));
        }
    } else if let Some(eq_pos) = trimmed.find('=') {
        let key = &trimmed[..eq_pos];
        let rest = &trimmed[eq_pos + 1..];
        spans.push(Span::styled(
            key.to_string(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw("=".to_string()));
        spans.push(Span::styled(
            rest.to_string(),
            Style::default().fg(Color::Green),
        ));
    } else {
        spans.push(Span::raw(trimmed.to_string()));
    }

    Line::from(spans)
}

fn highlight_pem(line: &str) -> Line<'_> {
    let trimmed = line.trim();
    if trimmed.starts_with("-----BEGIN") || trimmed.starts_with("-----END") {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if trimmed.is_empty() {
        return Line::from(line.to_string());
    }
    Line::from(Span::styled(
        line.to_string(),
        Style::default().fg(Color::DarkGray),
    ))
}

fn highlight_shell(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }

    let keywords = [
        "if ", "then ", "fi", "for ", "while ", "do ", "done ",
        "case ", "esac", "function ", "return ", "exit ",
        "local ", "export ", "declare ", "readonly ",
    ];

    let mut spans = Vec::new();
    let indent = &line[..line.len() - trimmed.len()];
    spans.push(Span::raw(indent.to_string()));

    let mut found_keyword = false;
    for kw in &keywords {
        if trimmed.starts_with(kw) {
            spans.push(Span::styled(
                kw.trim_end().to_string(),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(format!(" {}", &trimmed[kw.len()..])));
            found_keyword = true;
            break;
        }
    }

    if !found_keyword {
        // Check for $VAR patterns
        if trimmed.contains('$') {
            spans.push(Span::styled(
                trimmed.to_string(),
                Style::default().fg(Color::Yellow),
            ));
        } else if trimmed.contains('"') {
            spans.push(Span::styled(
                trimmed.to_string(),
                Style::default().fg(Color::Green),
            ));
        } else {
            spans.push(Span::raw(trimmed.to_string()));
        }
    }

    Line::from(spans)
}
