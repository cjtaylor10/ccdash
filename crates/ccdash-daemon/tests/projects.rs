mod common;

use common::Harness;
use tempfile::tempdir;

#[tokio::test]
async fn project_add_list_remove() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c
        .call("project.add", serde_json::json!({"path": proj.path()}))
        .await
        .unwrap();
    assert!(add.error.is_none(), "{:?}", add.error);
    let project = add.result.unwrap();
    let id = project["id"].as_str().unwrap().to_string();

    let list = c.call("project.list", serde_json::json!({})).await.unwrap();
    let projects = list.result.unwrap()["projects"].as_array().unwrap().clone();
    assert_eq!(projects.len(), 1);

    let rm = c
        .call("project.remove", serde_json::json!({"id": id}))
        .await
        .unwrap();
    assert!(rm.error.is_none());

    let list2 = c.call("project.list", serde_json::json!({})).await.unwrap();
    assert!(list2.result.unwrap()["projects"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn project_remove_unknown_returns_not_found() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let rm = c
        .call("project.remove", serde_json::json!({"id": "no-such-id"}))
        .await
        .unwrap();
    assert!(rm.error.is_some());
    assert_eq!(rm.error.unwrap().code, -32004);
}
