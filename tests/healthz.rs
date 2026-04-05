mod support;

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn get_healthz_returns_ok() {
    let temp = tempdir().unwrap();
    let db = temp.path().join("healthz.db");
    let engine = support::engine_empty(db).await;
    let app = support::app_with_engine(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), http::StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.as_ref(), b"ok");
}
