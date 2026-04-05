use axum::body::Body;
use bytes::Bytes;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support;

#[tokio::test]
async fn infisical_stub_webhook_fetches_and_writes_flat_files() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/auth/universal-auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accessToken": "stub-access-token"
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v3/secrets/raw"))
        .and(query_param("environment", "prod"))
        .and(query_param("secretPath", "/papra"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "secrets": [
                { "secretKey": "AUTH_SECRET", "secretValue": "value-from-stub" }
            ]
        })))
        .mount(&mock_server)
        .await;

    let temp = tempdir().unwrap();
    let db = temp.path().join("providers_stub.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();

    let cfg = support::infisical_webhook_app_config(mock_server.uri(), db, out.clone());
    let engine = support::engine_with_config_providers(cfg).await.unwrap();
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

    let secret_file = out.join("auth_secret");
    assert!(secret_file.is_file());
    assert_eq!(
        std::fs::read_to_string(secret_file).unwrap(),
        "value-from-stub"
    );
}
