//! ccdash desktop UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_state;
mod commands;
mod event_bridge;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .manage(client_state::ClientState::new())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                event_bridge::run(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_and_handshake,
            commands::project_list,
            commands::session_list,
            commands::ports_list,
            commands::plans_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
