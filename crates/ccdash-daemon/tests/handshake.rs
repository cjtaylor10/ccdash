mod common;

use common::Harness;

#[tokio::test]
async fn handshake_with_correct_token_succeeds() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = h.handshake(&mut c).await.unwrap();
    assert!(resp.error.is_none(), "got error: {:?}", resp.error);
    assert!(resp.result.unwrap()["protocol_version"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn handshake_with_wrong_token_fails() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = c.call("handshake", serde_json::json!({"token": "wrong", "client": "cli"})).await.unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32001);
}

#[tokio::test]
async fn pre_auth_method_call_is_rejected() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = c.call("project.list", serde_json::json!({})).await.unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32001);
}
