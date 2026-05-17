//! ccdash daemon entry point.

mod broadcast;
mod config;
mod plans;
mod ports;
mod projects;
mod rpc;
mod sessions;
mod state;
mod tmux;
mod worktrees;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

/// launchd / systemd give us a minimal PATH that may not include the
/// Homebrew bin dirs where `tmux` + `lsof` live. Augment PATH defensively
/// so the daemon can spawn its subprocesses regardless of how it was started.
fn augment_path() {
    const NEEDED: &[&str] = &[
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ];
    let existing = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = existing.split(':').collect();
    for need in NEEDED {
        if !parts.iter().any(|p| p == need) {
            parts.push(need);
        }
    }
    std::env::set_var("PATH", parts.join(":"));
}

/// Ensure a UTF-8 locale is set. Without LC_CTYPE/LANG, tmux's `-F` format
/// output replaces non-printable bytes (including TAB!) with underscores —
/// which silently breaks our pane parser. launchd strips locale env on
/// macOS, so this matters in practice. Default to C.UTF-8 if neither is set.
fn ensure_locale() {
    let has_locale = std::env::var("LC_ALL").is_ok()
        || std::env::var("LC_CTYPE").is_ok()
        || std::env::var("LANG").is_ok();
    if !has_locale {
        std::env::set_var("LC_CTYPE", "UTF-8");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    augment_path();
    ensure_locale();
    let cfg = config::Config::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&cfg.log_level).unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let state = state::AppState::bootstrap(cfg.resolved_data_dir()).await?;
    let socket = cfg.resolved_socket();

    // Spawn a graceful-shutdown task: on Ctrl-C or SIGTERM, remove the socket and exit.
    let shutdown_socket = socket.clone();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = term.recv() => {},
        }
        let _ = std::fs::remove_file(&shutdown_socket);
        std::process::exit(0);
    });

    rpc::serve(state, &socket).await
}
