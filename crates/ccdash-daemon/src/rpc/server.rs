//! Unix-socket listener and per-connection loop.

use crate::broadcast::Event;
use crate::rpc::codec::{FrameReader, FrameWriter};
use crate::rpc::dispatch::{dispatch, ConnContext};
use crate::state::AppState;
use anyhow::{Context, Result};
use ccdash_core::protocol::Notification;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub async fn serve(state: AppState, socket: &Path) -> Result<()> {
    // Remove stale socket if it exists.
    if socket.exists() {
        std::fs::remove_file(socket).with_context(|| format!("removing stale socket {}", socket.display()))?;
    }
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let listener = UnixListener::bind(socket).with_context(|| format!("binding {}", socket.display()))?;
    // chmod 0600
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(socket, perms).with_context(|| "setting socket permissions")?;
    info!(socket = %socket.display(), "ccdash-daemon listening");

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "accept failed");
                continue;
            }
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = connection(state, stream).await {
                warn!(error = %e, "connection ended");
            }
        });
    }
}

async fn connection(state: AppState, stream: tokio::net::UnixStream) -> Result<()> {
    let (r, w) = stream.into_split();
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    let ctx = Arc::new(RwLock::new(ConnContext::new()));

    // Subscribe to bus; we'll forward matching events as JSON-RPC notifications.
    let mut bus_rx = state.bus.subscribe();

    loop {
        tokio::select! {
            req = reader.next_request() => {
                match req {
                    Ok(Some(req)) => {
                        let resp = dispatch(req, &state, &ctx).await;
                        if let Err(e) = writer.write_response(&resp).await {
                            warn!(error = %e, "write failed; closing");
                            break;
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        warn!(error = %e, "frame read error; closing");
                        break;
                    }
                }
            }
            evt = bus_rx.recv() => {
                match evt {
                    Ok(event) => {
                        let topic = event.topic();
                        if ctx.read().await.subscriptions.contains(&topic) {
                            let n = Notification::new(method_for(&event), serde_json::to_value(&event).unwrap());
                            if let Err(e) = writer.write_notification(&n).await {
                                warn!(error = %e, "notification write failed");
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "subscriber lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

fn method_for(event: &Event) -> &'static str {
    match event {
        Event::ProjectsSnapshot { .. } => "projects.snapshot",
        Event::SessionsSnapshot { .. } => "sessions.snapshot",
        Event::ProjectUpdated { .. } => "project.updated",
        Event::ProjectRemoved { .. } => "project.removed",
        Event::SessionLaunched { .. } => "session.launched",
        Event::SessionUpdated { .. } => "session.updated",
        Event::SessionRemoved { .. } => "session.removed",
    }
}
