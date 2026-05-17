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
