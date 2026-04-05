use axum::body::Body;
use bytes::Bytes;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

use crate::support;

#[tokio::test]
async fn webhook_valid_signature_applies_service() {
    let temp = tempdir().unwrap();
    let db = temp.path().join("happy.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();
    let cfg = support::webhook_sample_app_config(db, out.clone());
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let engine = support::engine_with_webhook_mock(cfg, calls.clone()).await;
    let app = support::app_with_engine(engine);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);
    let headers = support::signed_headers("top-secret", &body);
    let mut req_builder = Request::builder()
        .method("POST")
        .uri("/webhooks/infisical_main")
        .header("content-type", "application/json");
    for (name, value) in headers.iter() {
        req_builder = req_builder.header(name, value);
    }
    let response = app
        .oneshot(req_builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), http::StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["matched_services"], 1);
    assert_eq!(v["applied_services"], 1);
    assert_eq!(v["skipped_duplicate"], false);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    let secret_file = out.join("auth_secret");
    assert!(secret_file.is_file());
    assert_eq!(std::fs::read_to_string(secret_file).unwrap(), "value-1");
}
