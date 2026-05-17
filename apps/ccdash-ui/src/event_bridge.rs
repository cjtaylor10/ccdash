//! Subscribes the shared Client to daemon broadcast notifications and
//! re-emits them as Tauri events (`daemon-event`) so all windows hear them.

use crate::client_state::ClientState;
use ccdash_core::protocol::Topic;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, warn};

pub async fn run(app: AppHandle) {
    let state: tauri::State<'_, ClientState> = app.state();
    let mut waited = 0u64;
    loop {
        if state.inner.lock().await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += 100;
        if waited >= 30_000 {
            warn!("daemon event bridge: client never connected — giving up");
            return;
        }
    }

    {
        let mut guard = state.inner.lock().await;
        let client = guard.as_mut().unwrap();
        match client
            .subscribe(vec![
                Topic::Projects,
                Topic::Sessions,
                Topic::Ports,
                Topic::Plans,
            ])
            .await
        {
            Ok(resp) if resp.error.is_some() => {
                warn!("subscribe error: {:?}", resp.error);
                return;
            }
            Err(e) => {
                warn!("subscribe failed: {}", e);
                return;
            }
            _ => {}
        }
    }

    loop {
        let next = {
            let mut guard = state.inner.lock().await;
            let client = guard.as_mut().unwrap();
            client.next_notification().await
        };
        match next {
            Ok(v) => {
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
                    warn!("emit failed: {}", e);
                }
            }
            Err(e) => {
                warn!("notification read failed: {} — bridge exiting", e);
                return;
            }
        }
    }
}
