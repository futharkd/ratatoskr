use axum::body::Body;
use bytes::Bytes;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

use crate::support;

async fn post_signed(app: axum::Router, body: &Bytes) -> (http::StatusCode, serde_json::Value) {
    let mut req_builder = Request::builder()
        .method("POST")
        .uri("/webhooks/infisical_main")
        .header("content-type", "application/json");
    let headers = support::signed_headers("top-secret", body);
    for (name, value) in headers.iter() {
        req_builder = req_builder.header(name, value);
    }
    let response = app
        .oneshot(req_builder.body(Body::from(body.clone())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, v)
}

#[tokio::test]
async fn duplicate_delivery_skips_second_apply() {
    let temp = tempdir().unwrap();
    let db = temp.path().join("idem.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();
    let cfg = support::webhook_sample_app_config(db, out);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let engine = support::engine_with_webhook_mock(cfg, calls.clone()).await;
    let app = support::app_with_engine(engine);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);

    let (s1, v1) = post_signed(app.clone(), &body).await;
    assert_eq!(s1, http::StatusCode::OK);
    assert_eq!(v1["applied_services"], 1);
    assert_eq!(v1["skipped_duplicate"], false);

    let (s2, v2) = post_signed(app, &body).await;
    assert_eq!(s2, http::StatusCode::OK);
    assert_eq!(v2["applied_services"], 0);
    assert_eq!(v2["skipped_duplicate"], true);

    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
