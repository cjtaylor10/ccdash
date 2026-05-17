//! Tauri commands that proxy to ccdash-daemon via a shared `Client`.

use crate::client_state::ClientState;
use ccdash_core::client::Client;
use ccdash_core::protocol::ClientKind;
use serde_json::Value;
use tauri::State;

#[tauri::command]
pub async fn connect_and_handshake(state: State<'_, ClientState>) -> Result<String, String> {
    let mut guard = state.inner.lock().await;
    if guard.is_some() {
        return Ok("already connected".into());
    }
    let mut client = Client::connect_default()
        .await
        .map_err(|e| format!("connect failed: {}", e))?;
    let resp = client
        .handshake(ClientKind::Ui)
        .await
        .map_err(|e| format!("handshake failed: {}", e))?;
    if let Some(err) = resp.error {
        return Err(format!("handshake rejected: {}", err.message));
    }
    *guard = Some(client);
    Ok("connected".into())
}

async fn call_method(
    state: &State<'_, ClientState>,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let mut guard = state.inner.lock().await;
    let client = guard
        .as_mut()
        .ok_or_else(|| "daemon not connected — call connect_and_handshake first".to_string())?;
    let resp = client
        .call(method, params)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(err) = resp.error {
        return Err(err.message);
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn project_list(state: State<'_, ClientState>) -> Result<Value, String> {
    call_method(&state, "project.list", serde_json::json!({})).await
}

#[tauri::command]
pub async fn session_list(state: State<'_, ClientState>) -> Result<Value, String> {
    call_method(&state, "session.list", serde_json::json!({})).await
}

#[tauri::command]
pub async fn ports_list(state: State<'_, ClientState>) -> Result<Value, String> {
    call_method(&state, "ports.list", serde_json::json!({})).await
}

#[tauri::command]
pub async fn plans_get(state: State<'_, ClientState>, project_id: String) -> Result<Value, String> {
    call_method(
        &state,
        "plans.get",
        serde_json::json!({"project_id": project_id}),
    )
    .await
}

// === Terminal commands ===

use crate::pty::PtyManager;

#[tauri::command]
pub async fn terminal_open(
    app: tauri::AppHandle,
    pty: tauri::State<'_, PtyManager>,
    command: Vec<String>,
    rows: u16,
    cols: u16,
) -> Result<String, String> {
    pty.open(app, command, rows, cols).await
}

#[tauri::command]
pub async fn terminal_write(
    pty: tauri::State<'_, PtyManager>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    pty.write(&id, &data).await
}

#[tauri::command]
pub async fn terminal_resize(
    pty: tauri::State<'_, PtyManager>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    pty.resize(&id, rows, cols).await
}

#[tauri::command]
pub async fn terminal_close(pty: tauri::State<'_, PtyManager>, id: String) -> Result<(), String> {
    pty.close(&id).await
}

// === Window commands ===

#[tauri::command]
pub async fn open_new_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::windows::open_new_window(&app)
}

#[tauri::command]
pub async fn list_windows(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Manager;
    Ok(app.webview_windows().into_keys().collect::<Vec<_>>())
}

#[tauri::command]
pub async fn publish_window_state(
    app: tauri::AppHandle,
    from: String,
    state: serde_json::Value,
) -> Result<(), String> {
    use tauri::Emitter;
    app.emit(&format!("window-state-broadcast::{}", from), state)
        .map_err(|e| e.to_string())
}
