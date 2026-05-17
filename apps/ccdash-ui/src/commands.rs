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

// === Project management ===

#[tauri::command]
pub async fn project_add(
    state: State<'_, ClientState>,
    path: String,
    name: Option<String>,
) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    params.insert("path".into(), Value::String(path));
    if let Some(n) = name {
        params.insert("name".into(), Value::String(n));
    }
    call_method(&state, "project.add", Value::Object(params)).await
}

#[tauri::command]
pub async fn project_remove(state: State<'_, ClientState>, id: String) -> Result<Value, String> {
    call_method(&state, "project.remove", serde_json::json!({ "id": id })).await
}

#[tauri::command]
pub async fn session_launch(
    state: State<'_, ClientState>,
    project_id: String,
    worktree: Option<String>,
    command: Option<String>,
    force_token: Option<String>,
) -> Result<Value, String> {
    let mut params = serde_json::Map::new();
    params.insert("project_id".into(), Value::String(project_id));
    if let Some(w) = worktree {
        params.insert("worktree".into(), Value::String(w));
    }
    if let Some(c) = command {
        params.insert("command".into(), Value::String(c));
    }
    if let Some(t) = force_token {
        params.insert("force_token".into(), Value::String(t));
    }
    call_method(&state, "session.launch", Value::Object(params)).await
}

#[tauri::command]
pub async fn session_kill(
    state: State<'_, ClientState>,
    tmux_session_id: String,
) -> Result<Value, String> {
    call_method(
        &state,
        "session.kill",
        serde_json::json!({ "tmux_session_id": tmux_session_id }),
    )
    .await
}

/// Diagnostic: write a message to the daemon's ui log file. Used by the
/// frontend to surface JS errors that would otherwise be invisible.
#[tauri::command]
pub async fn log_from_frontend(level: String, message: String) -> Result<(), String> {
    match level.as_str() {
        "error" => tracing::error!(target: "ccdash_ui_frontend", "{}", message),
        "warn" => tracing::warn!(target: "ccdash_ui_frontend", "{}", message),
        _ => tracing::info!(target: "ccdash_ui_frontend", "{}", message),
    }
    Ok(())
}
