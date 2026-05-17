mod common;

use common::Harness;
use std::process::Command;
use tempfile::tempdir;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn launch_then_kill_session() {
    if !tmux_available() {
        eprintln!("tmux not on PATH; skipping");
        return;
    }
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c
        .call(
            "project.add",
            serde_json::json!({"path": proj.path(), "name": "smoke"}),
        )
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let launch = c
        .call(
            "session.launch",
            serde_json::json!({
                "project_id": project_id,
                "worktree": null,
                "command": "sleep 30",
            }),
        )
        .await
        .unwrap();
    assert!(launch.error.is_none(), "launch error: {:?}", launch.error);
    let session = launch.result.unwrap()["session"].clone();
    let sid = session["tmux_session_id"].as_str().unwrap().to_string();
    assert!(sid.starts_with('$'));

    // session.list filters by pane_current_command == "claude" per spec §7.7.
    // Our test uses "sleep" as the command, so it shouldn't appear in the list.
    // Verifying launch/kill round-trip is enough for Phase 1.
    let _ = c.call("session.list", serde_json::json!({})).await.unwrap();

    let kill = c
        .call("session.kill", serde_json::json!({"tmux_session_id": sid}))
        .await
        .unwrap();
    assert!(kill.error.is_none(), "kill error: {:?}", kill.error);
}
