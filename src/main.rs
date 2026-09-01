mod app;
mod application;
mod annotations;
mod config;
mod domain;
mod infra;
mod knowledge;
mod syntax;
mod ui;

use app::App;
use clap::{Parser, Subcommand};
use crate::domain::ports::RegistryStore;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
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
    /// Initialize a new config/index at a path (default: ./arioch)
    Init {
        #[arg(default_value = "./arioch")]
        path: String,
    },
    /// Export index to a JSON file
    Export {
        #[arg(short, long, default_value = "arioch-index.json")]
        output: String,
    },
    /// Import entries from a JSON file
    Import {
        file: String,
        /// Replace existing entries instead of merging
        #[arg(short, long)]
        replace: bool,
    },
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

    // CLI subcommand → operate on the index through the store port
    let config_dir = config::active_config_dir();
    let store = infra::index_store::TomlIndex::new(&config_dir);
    let mut entries = store.load()?;
    match cli.command.unwrap() {
        Command::List => application::cli::cmd_list(&entries, cli.json),
        Command::Add {
            path,
            category,
            tags,
            description,
            alias,
        } => {
            application::cli::cmd_add(
                &mut entries,
                &path,
                category,
                tags,
                description,
                alias,
                cli.json,
                &store,
            )
        }
        Command::Remove { path } => {
            application::cli::cmd_remove(&mut entries, &path, cli.json, &store)
        }
        Command::Tag { path, tag } => {
            application::cli::cmd_tag(&mut entries, &path, &tag, cli.json, &store)
        }
        Command::Map => application::cli::cmd_map(&entries, cli.json),
        Command::Scan => application::cli::cmd_scan(&infra::fs::RealFs, &config::Config::load(), cli.json),
        Command::Init { path } => application::cli::cmd_init(&path),
        Command::Export { output } => application::cli::cmd_export(&entries, &output),
        Command::Import { file, replace } => {
            application::cli::cmd_import(&mut entries, &file, replace, &store)
        }
    }
}

// ─── TUI (no subcommand) ───────────────────────────────────────────────────

fn run_tui() -> anyhow::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let config = config::Config::load();
    // audit log watches the raw config dir (today's behavior); registry and
    // annotations honor the --config override
    let audit_dir = config::Config::config_dir();
    let active_dir = config::active_config_dir();
    let ports = app::AppPorts {
        fs: Box::new(infra::fs::RealFs),
        editor: Box::new(infra::process::ShellEditor::new(config.editor())),
        clipboard: Box::new(infra::process::SystemClipboard),
        audit: Box::new(infra::audit_log::FileAuditLog::new(&audit_dir)),
        registry_store: Box::new(infra::index_store::TomlIndex::new(&active_dir)),
        annotation_store: Box::new(infra::annotations_store::TomlAnnotations::new(&active_dir)),
    };
    let entries = ports.registry_store.load().unwrap_or_default();
    let mut app = App::new(config, entries, ports);

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

