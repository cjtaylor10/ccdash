mod common;

use common::Harness;
use std::net::TcpListener;
use tempfile::tempdir;

fn tmux_available() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn ports_list_succeeds() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let resp = c.call("ports.list", serde_json::json!({})).await.unwrap();
    assert!(resp.error.is_none(), "ports.list error: {:?}", resp.error);
    let result = resp.result.unwrap();
    assert!(result["running"].is_array());
    assert!(result["declared"].is_array());
}

#[tokio::test]
async fn session_launch_conflict_returns_force_token() {
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

    // Pick an ephemeral port, then re-bind it for the duration of the test.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let held = TcpListener::bind(("127.0.0.1", port)).unwrap();

    // Create a project whose .env declares that port.
    let proj = tempdir().unwrap();
    std::fs::write(proj.path().join(".env"), format!("PORT={}\n", port)).unwrap();
    let add = c
        .call(
            "project.add",
            serde_json::json!({"path": proj.path(), "name": "conflict-test"}),
        )
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    // Attempt to launch — should be blocked.
    let resp = c
        .call(
            "session.launch",
            serde_json::json!({"project_id": project_id, "command": "sleep 30"}),
        )
        .await
        .unwrap();
    let err = resp.error.expect("expected error");
    assert_eq!(err.code, -32002);
    let data = err.data.expect("expected conflict data");
    let token = data["force_token"]
        .as_str()
        .expect("force_token")
        .to_string();
    assert!(!token.is_empty());
    let conflicts = data["conflicts"].as_array().unwrap();
    assert!(conflicts
        .iter()
        .any(|c| c["port"].as_u64() == Some(port as u64)));

    // Re-launch with the force token — should succeed.
    let resp2 = c
        .call(
            "session.launch",
            serde_json::json!({"project_id": project_id, "command": "sleep 30", "force_token": token}),
        )
        .await
        .unwrap();
    assert!(
        resp2.error.is_none(),
        "force-launch failed: {:?}",
        resp2.error
    );
    let sid = resp2.result.unwrap()["session"]["tmux_session_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Cleanup.
    let _ = c
        .call("session.kill", serde_json::json!({"tmux_session_id": sid}))
        .await;
    drop(held);
}
