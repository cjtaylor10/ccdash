//! Screenshot-to-clipboard helpers.
//!
//! Uses macOS `screencapture -c` (clipboard) with `-R<x,y,w,h>` so we can
//! target either the entire app window or an arbitrary sub-region (e.g. the
//! BrowserView iframe). `-R` expects *logical* pixels, so we divide the
//! window's physical position/size by its scale factor.
//!
//! macOS Screen Recording permission is gated through TCC. We call
//! `CGRequestScreenCaptureAccess` directly so the system prompt is attributed
//! to ccdash-ui (rather than to whatever spawned `screencapture` — TCC's
//! attribution heuristics for subprocess capture are unreliable). The grant
//! only takes effect on next process launch, so we surface a clear "grant +
//! relaunch" message when the preflight fails.

use tauri::{Manager, WebviewWindow};

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// True if this process currently has Screen Recording permission. Cheap —
/// no prompt, no side effects.
#[cfg(target_os = "macos")]
pub fn has_screen_recording_permission() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Trigger the macOS Screen Recording permission prompt for this process if
/// no decision has been recorded yet. Returns the current state immediately;
/// the prompt itself is async and the user's choice only takes effect after
/// the next launch of this process.
#[cfg(target_os = "macos")]
pub fn request_screen_recording_permission() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
pub fn has_screen_recording_permission() -> bool {
    true
}
#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording_permission() -> bool {
    true
}

#[cfg(target_os = "macos")]
async fn screencapture_region(x: i32, y: i32, w: i32, h: i32) -> Result<(), String> {
    if w <= 0 || h <= 0 {
        return Err(format!("invalid region: {}x{}", w, h));
    }
    if !has_screen_recording_permission() {
        // Will either pop the system prompt (first time) or no-op (denied).
        // Either way, the grant only takes effect on next launch.
        let _ = request_screen_recording_permission();
        tracing::warn!("screen recording permission not granted; prompted user");
        return Err(
            "Screen Recording permission is required. macOS should have just \
             shown a prompt (or you can add it manually in System Settings → \
             Privacy & Security → Screen Recording, with ccdash-ui ticked). \
             Then fully quit and relaunch ccdash for the grant to take effect."
                .into(),
        );
    }
    let rect = format!("{},{},{},{}", x, y, w, h);
    tracing::info!("screencapture -c -x -R {}", rect);
    let output = tokio::process::Command::new("screencapture")
        .args(["-c", "-x", "-R", &rect])
        .output()
        .await
        .map_err(|e| {
            tracing::error!("spawn screencapture failed: {}", e);
            format!("spawn screencapture: {}", e)
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::error!(
            "screencapture failed: status={:?} stderr={:?} stdout={:?}",
            output.status.code(),
            stderr,
            stdout
        );
        let detail = if stderr.is_empty() { stdout } else { stderr };
        let hint = if detail.contains("could not create image") {
            " (grant Screen Recording permission to ccdash-ui in System Settings → Privacy & Security, then quit and relaunch the app)"
        } else {
            ""
        };
        return Err(format!("screencapture failed: {}{}", detail, hint));
    }
    tracing::info!("screencapture ok");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
async fn screencapture_region(_x: i32, _y: i32, _w: i32, _h: i32) -> Result<(), String> {
    Err("screenshot-to-clipboard is only implemented on macOS for now".into())
}

fn rect_for_window(window: &WebviewWindow) -> Result<(i32, i32, i32, i32), String> {
    let pos = window
        .outer_position()
        .map_err(|e| format!("outer_position: {}", e))?;
    let size = window
        .outer_size()
        .map_err(|e| format!("outer_size: {}", e))?;
    let scale = window
        .scale_factor()
        .map_err(|e| format!("scale_factor: {}", e))?;
    let x = (pos.x as f64 / scale).round() as i32;
    let y = (pos.y as f64 / scale).round() as i32;
    let w = (size.width as f64 / scale).round() as i32;
    let h = (size.height as f64 / scale).round() as i32;
    Ok((x, y, w, h))
}

/// Screenshot the entire window the call originated from, copying the
/// resulting PNG to the system clipboard.
#[tauri::command]
pub async fn screenshot_window(
    app: tauri::AppHandle,
    label: Option<String>,
) -> Result<(), String> {
    let win = match label {
        Some(l) => app
            .webview_windows()
            .get(&l)
            .cloned()
            .ok_or_else(|| format!("no such window: {}", l))?,
        None => app
            .webview_windows()
            .values()
            .next()
            .cloned()
            .ok_or_else(|| "no windows available".to_string())?,
    };
    let (x, y, w, h) = rect_for_window(&win)?;
    screencapture_region(x, y, w, h).await
}

/// Screenshot an arbitrary rect within a window's content area. The
/// coordinates are CSS (logical) pixels relative to the window's inner
/// (client) area — i.e. what `element.getBoundingClientRect()` returns.
#[tauri::command]
pub async fn screenshot_region(
    app: tauri::AppHandle,
    label: Option<String>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let win = match label {
        Some(l) => app
            .webview_windows()
            .get(&l)
            .cloned()
            .ok_or_else(|| format!("no such window: {}", l))?,
        None => app
            .webview_windows()
            .values()
            .next()
            .cloned()
            .ok_or_else(|| "no windows available".to_string())?,
    };
    let inner_pos = win
        .inner_position()
        .map_err(|e| format!("inner_position: {}", e))?;
    let scale = win
        .scale_factor()
        .map_err(|e| format!("scale_factor: {}", e))?;
    let win_x = inner_pos.x as f64 / scale;
    let win_y = inner_pos.y as f64 / scale;
    let sx = (win_x + x).round() as i32;
    let sy = (win_y + y).round() as i32;
    let sw = width.round() as i32;
    let sh = height.round() as i32;
    screencapture_region(sx, sy, sw, sh).await
}
