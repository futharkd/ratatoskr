use axum::body::Body;
use bytes::Bytes;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

use crate::support;

#[tokio::test]
async fn webhook_missing_signature_returns_400() {
    let temp = tempdir().unwrap();
    let db = temp.path().join("bad.db");
    let out = temp.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    let cfg = support::webhook_sample_app_config(db, out);
    let engine = support::engine_with_webhook_mock(
        cfg,
        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    )
    .await;
    let app = support::app_with_engine(engine);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhooks/infisical_main")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.get("error").is_some());
}
