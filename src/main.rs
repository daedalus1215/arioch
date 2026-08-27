mod app;
mod config;
mod knowledge;
mod registry;
mod syntax;
mod ui;

use app::App;
use clap::{Parser, Subcommand};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use registry::{Entry, Registry};
use std::io;

#[derive(Parser)]
#[command(name = "arioch", about = "Security file manager — view, manage, and map local security files")]
struct Cli {
    /// Use non-default config/index location
    #[arg(long, global = true)]
    config: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List all registered entries
    List,
    /// Add a new entry
    Add {
        path: String,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(short, long)]
        tags: Option<String>,
        #[arg(short, long)]
        description: Option<String>,
        #[arg(short, long)]
        alias: Option<String>,
    },
    /// Remove an entry by path
    Remove {
        path: String,
    },
    /// Add a tag to an entry
    Tag {
        path: String,
        tag: String,
    },
    /// Print the relationship map to stdout
    Map,
    /// Run scan and print suggestions
    Scan,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Apply --config override before any loading
    if let Some(ref config_path) = cli.config {
        let expanded = shellexpand::tilde(config_path);
        config::set_config_override(std::path::PathBuf::from(expanded.into_owned()));
    }

    // No subcommand → launch TUI
    if cli.command.is_none() {
        setup_terminal()?;
        let result = run_tui();
        restore_terminal()?;
        return result;
    }

    // CLI subcommand → operate on registry directly
    let mut registry = Registry::load()?;
    match cli.command.unwrap() {
        Command::List => cmd_list(&registry, cli.json),
        Command::Add {
            path,
            category,
            tags,
            description,
            alias,
        } => cmd_add(&mut registry, &path, category, tags, description, alias, cli.json),
        Command::Remove { path } => cmd_remove(&mut registry, &path, cli.json),
        Command::Tag { path, tag } => cmd_tag(&mut registry, &path, &tag, cli.json),
        Command::Map => cmd_map(&registry, cli.json),
        Command::Scan => cmd_scan(&mut registry, cli.json),
    }
}

fn cmd_list(registry: &Registry, json: bool) -> anyhow::Result<()> {
    if json {
        let entries: Vec<serde_json::Value> = registry
            .entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "category": e.category,
                    "tags": e.tags,
                    "description": e.description,
                    "alias": e.alias,
                    "related": e.related,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        if registry.entries.is_empty() {
            println!("No entries registered.");
            return Ok(());
        }
        for e in &registry.entries {
            let tags = if e.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.tags.join(", "))
            };
            let alias = e.alias.as_deref().map(|a| format!(" (alias: {})", a)).unwrap_or_default();
            println!("{}{}{}{}", e.path, tags, alias, if e.category.is_empty() {
                String::new()
            } else {
                format!("  # {}", e.category)
            });
        }
    }
    Ok(())
}

fn cmd_add(
    registry: &mut Registry,
    path: &str,
    category: Option<String>,
    tags: Option<String>,
    description: Option<String>,
    alias: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    if !expanded.exists() {
        anyhow::bail!("File not found: {}", path);
    }

    let tag_vec: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let entry = Entry {
        path: path.to_string(),
        category: category.unwrap_or_else(|| guess_category(&path)),
        tags: tag_vec,
        description: description.unwrap_or_default(),
        alias,
        related: Vec::new(),
    };

    registry.entries.push(entry);
    registry.save()?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": path,
                "category": registry.entries.last().unwrap().category,
                "tags": registry.entries.last().unwrap().tags,
                "description": registry.entries.last().unwrap().description,
            })
        );
    } else {
        println!("Added: {} ({})", path, registry.entries.last().unwrap().category);
    }
    Ok(())
}

fn cmd_remove(registry: &mut Registry, path: &str, json: bool) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    let idx = registry
        .entries
        .iter()
        .position(|e| expand_path(&e.path) == expanded || e.path == path);

    match idx {
        Some(i) => {
            let entry = registry.entries[i].clone();
            registry.entries.remove(i);
            registry.save()?;
            if json {
                println!("{}", serde_json::json!({"removed": entry.path}));
            } else {
                println!("Removed: {}", entry.path);
            }
        }
        None => {
            anyhow::bail!("Entry not found: {}", path);
        }
    }
    Ok(())
}

fn cmd_tag(registry: &mut Registry, path: &str, tag: &str, json: bool) -> anyhow::Result<()> {
    let expanded = expand_path(path);
    let idx = registry
        .entries
        .iter()
        .position(|e| expand_path(&e.path) == expanded || e.path == path);

    match idx {
        Some(i) => {
            if !registry.entries[i].tags.iter().any(|t| t == tag) {
                registry.entries[i].tags.push(tag.to_string());
            }
            registry.save()?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "path": registry.entries[i].path,
                        "tags": registry.entries[i].tags,
                    })
                );
            } else {
                println!(
                    "Tagged: {} [{}]",
                    registry.entries[i].path,
                    registry.entries[i].tags.join(", ")
                );
            }
        }
        None => {
            anyhow::bail!("Entry not found: {}", path);
        }
    }
    Ok(())
}

fn cmd_map(registry: &Registry, json: bool) -> anyhow::Result<()> {
    if json {
        let nodes: Vec<serde_json::Value> = registry
            .entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.alias.as_deref().unwrap_or_else(|| e.path.rsplit('/').next().unwrap_or(&e.path)),
                    "category": e.category,
                    "related": e.related,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else {
        for entry in &registry.entries {
            let name = entry
                .alias
                .as_deref()
                .unwrap_or_else(|| entry.path.rsplit('/').next().unwrap_or(&entry.path));
            println!("[{}]", name);
            for rel in &entry.related {
                println!("  ──▶ {}", rel);
            }
        }
    }
    Ok(())
}

fn cmd_scan(registry: &mut Registry, json: bool) -> anyhow::Result<()> {
    let config = config::Config::load();
    registry.scan_with_config(
        &config.scan_paths,
        &config.exclude_paths,
        &config.scan_patterns,
        config.scan_depth,
    );

    if json {
        let suggestions: Vec<String> = registry
            .suggestions
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
    } else {
        if registry.suggestions.is_empty() {
            println!("No suggestions found.");
        } else {
            println!("Found {} potential security files:", registry.suggestions.len());
            for s in &registry.suggestions {
                println!("  {}", s.to_string_lossy());
            }
        }
    }
    Ok(())
}

fn expand_path(path: &str) -> std::path::PathBuf {
    let expanded = shellexpand::tilde(path);
    std::path::PathBuf::from(expanded.into_owned())
}

fn guess_category(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains(".ssh") || lower.contains("id_") {
        "ssh-keys".to_string()
    } else if lower.contains("cert") || lower.contains(".pem") || lower.contains(".crt") {
        "certs".to_string()
    } else if lower.contains("credential") || lower.contains("token") || lower.contains("secret") {
        "creds".to_string()
    } else if lower.contains("config") || lower.contains(".toml") || lower.contains(".yaml") || lower.contains(".yml") {
        "configs".to_string()
    } else {
        "other".to_string()
    }
}

// ─── TUI (no subcommand) ───────────────────────────────────────────────────

fn run_tui() -> anyhow::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        match App::next_event() {
            app::Event::Key(key) => {
                app.handle_key(key);
                if app.quit {
                    break;
                }
            }
            app::Event::Tick => {
                app.tick();
            }
        }
    }

    Ok(())
}

fn setup_terminal() -> anyhow::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
        crossterm::terminal::EnableLineWrap,
    )?;
    Ok(())
}

fn restore_terminal() -> anyhow::Result<()> {
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
        crossterm::terminal::DisableLineWrap,
    )?;
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}
