mod common;

use common::Harness;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn subscribed_client_receives_project_updated_notification() {
    let h = Harness::spawn().await.unwrap();

    // Client A: subscribes to projects.
    let mut a = h.connect().await.unwrap();
    h.handshake(&mut a)
        .await
        .unwrap()
        .result
        .expect("handshake ok");
    let sub = a
        .call("subscribe", serde_json::json!({"topics": ["projects"]}))
        .await
        .unwrap();
    assert!(sub.error.is_none());

    // Client B: triggers a project.add.
    let mut b = h.connect().await.unwrap();
    h.handshake(&mut b)
        .await
        .unwrap()
        .result
        .expect("handshake ok");
    let proj = tempdir().unwrap();
    let _ = b
        .call("project.add", serde_json::json!({"path": proj.path()}))
        .await
        .unwrap();

    // Client A should now see a project.updated notification (no id, has method).
    let next_line = read_next_line(&mut a).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&next_line).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert!(v["method"].as_str().unwrap().starts_with("project."));
    assert!(
        v.get("id").is_none() || v["id"].is_null(),
        "notifications must have no id"
    );
}

async fn read_next_line(c: &mut common::Conn) -> anyhow::Result<String> {
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();
    timeout(Duration::from_secs(2), c.reader().read_line(&mut line)).await??;
    Ok(line)
}
