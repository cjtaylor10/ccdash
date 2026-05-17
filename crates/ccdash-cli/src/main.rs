//! `ccdash` CLI — connects to ccdash-daemon over Unix socket.

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ccdash", version, about = "ccdash command-line client")]
struct Cli {
    /// Override the daemon socket path.
    #[arg(long, env = "CCDASH_SOCKET", global = true)]
    socket: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, env = "CCDASH_LOG", default_value = "warn", global = true)]
    log_level: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show daemon health + counts.
    Status,
    /// Project management.
    Project {
        #[command(subcommand)]
        sub: commands::project::Sub,
    },
    /// List running sessions.
    List,
    /// Launch a new session.
    Launch {
        /// Project name or ID.
        project: String,
        /// Worktree name (defaults to primary).
        #[arg(long)]
        worktree: Option<String>,
        /// Command override (defaults to `claude`).
        #[arg(long)]
        command: Option<String>,
        /// Bypass port-conflict check (use the force_token returned from a prior conflict).
        #[arg(long)]
        force_token: Option<String>,
    },
    /// Kill a session by tmux session_id (e.g. `$3`).
    Kill { session_id: String },
    /// Show port registry.
    Ports {
        /// Filter to one project.
        #[arg(long)]
        project: Option<String>,
    },
    /// Show plan progress for a project.
    Plan {
        /// Project name or ID.
        project: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    match cli.cmd {
        Command::Status => commands::status::run(cli.socket).await,
        Command::Project { sub } => commands::project::run(cli.socket, sub).await,
        Command::List => commands::list::run(cli.socket).await,
        Command::Launch {
            project,
            worktree,
            command,
            force_token,
        } => commands::launch::run(cli.socket, project, worktree, command, force_token).await,
        Command::Kill { session_id } => commands::kill::run(cli.socket, session_id).await,
        Command::Ports { project } => commands::ports::run(cli.socket, project).await,
        Command::Plan { project } => commands::plan::run(cli.socket, project).await,
    }
}
