//! ccdash desktop UI entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client_state;
mod commands;
mod event_bridge;
mod pty;
mod window_clamp;
mod windows;

use tracing_subscriber::EnvFilter;

fn main() {
    // Log to ~/.ccdash/ui.log so the GUI app's logs are inspectable even when
    // launched via `open` (where stdout/stderr are inaccessible).
    let log_dir = ccdash_core::paths::data_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("ui.log"))
        .ok();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match log_file {
        Some(f) => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::sync::Mutex::new(f))
                .with_ansi(false)
                .init();
        }
        None => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }
    tracing::info!("ccdash-ui starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Moved(_) = event {
                window_clamp::clamp_window_position(window);
            }
        })
        .manage(client_state::ClientState::new())
        .manage(pty::PtyManager::new())
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
            commands::first_run_status,
            commands::first_run_complete,
            commands::scan_paths,
            commands::project_add,
            commands::project_remove,
            commands::project_reorder,
            commands::session_launch,
            commands::session_kill,
            commands::terminal_open,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
            commands::open_new_window,
            commands::list_windows,
            commands::publish_window_state,
            commands::log_from_frontend,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
