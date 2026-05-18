//! Helpers for creating + addressing additional app windows.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_new_window(app: &AppHandle) -> Result<(), String> {
    let count = app.webview_windows().len();
    let label = format!("ccdash-{}", count + 1);
    let w = WebviewWindowBuilder::new(app, &label, WebviewUrl::default())
        .title(format!("ccdash ({})", count + 1))
        .inner_size(1100.0, 720.0)
        .center()
        .build()
        .map_err(|e| format!("window: {}", e))?;
    let win = w.as_ref().window();
    crate::window_clamp::clamp_window_position(&win);
    Ok(())
}

/// Open a popped-out terminal window dedicated to a single tmux session.
/// The window loads the same Svelte app but with a `?term=<sessionId>`
/// query string; App.svelte renders a terminal-only mode that auto-attaches.
/// These windows survive the main window closing (each Tauri window has
/// independent lifecycle) and the underlying tmux session is unaffected
/// regardless of which windows are open.
pub fn open_terminal_window(
    app: &AppHandle,
    session_id: &str,
    session_name: &str,
) -> Result<(), String> {
    let count = app.webview_windows().len();
    let label = format!("ccdash-term-{}-{}", count + 1, sanitize_label(session_id));
    let url = format!("index.html?term={}", urlencoding::encode(session_id));
    let w = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("{}  —  ccdash", session_name))
        .inner_size(900.0, 600.0)
        .center()
        .build()
        .map_err(|e| format!("popout window: {}", e))?;
    let win = w.as_ref().window();
    crate::window_clamp::clamp_window_position(&win);
    Ok(())
}

/// Tauri window labels must be alphanumeric + `-_/:`. Tmux session ids
/// like `$0` contain `$` which isn't allowed.
fn sanitize_label(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
