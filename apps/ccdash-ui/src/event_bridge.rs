//! Subscribes a dedicated daemon Client to broadcast notifications and
//! re-emits them as Tauri events (`daemon-event`) so all windows hear them.
//!
//! IMPORTANT: this uses its OWN Client connection instead of sharing the one
//! held in ClientState. Sharing would deadlock: the forwarding loop calls
//! `next_notification().await` while holding the mutex, blocking any window
//! that opens later and tries to call `connect_and_handshake`.

use ccdash_core::client::Client;
use ccdash_core::protocol::{ClientKind, Topic};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, warn};

pub async fn run(app: AppHandle) {
    // Reconnect loop so the bridge survives daemon restarts.
    loop {
        match run_once(&app).await {
            Ok(()) => {
                info!("event bridge: clean exit");
                return;
            }
            Err(e) => {
                warn!(error = %e, "event bridge: lost connection, retrying in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn run_once(app: &AppHandle) -> anyhow::Result<()> {
    let mut client = Client::connect_default().await?;
    let resp = client.handshake(ClientKind::Ui).await?;
    if let Some(err) = resp.error {
        anyhow::bail!("event bridge handshake rejected: {}", err.message);
    }
    let sub_resp = client
        .subscribe(vec![
            Topic::Projects,
            Topic::Sessions,
            Topic::Ports,
            Topic::Plans,
        ])
        .await?;
    if let Some(err) = sub_resp.error {
        anyhow::bail!("event bridge subscribe rejected: {}", err.message);
    }
    info!("event bridge: subscribed to all topics");

    loop {
        let v = client.next_notification().await?;
        let method = v
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let params = v.get("params").cloned().unwrap_or(serde_json::Value::Null);
        debug!(method = %method, "forwarding daemon notification");
        if let Err(e) = app.emit(
            "daemon-event",
            serde_json::json!({"method": method, "params": params}),
        ) {
            warn!(error = %e, "emit failed");
        }
    }
}
