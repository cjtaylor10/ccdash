//! ccdash daemon entry point.

mod broadcast;
mod config;
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

#[tokio::main]
async fn main() -> Result<()> {
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
