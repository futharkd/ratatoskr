//! Hermetic Infisical provider tests: wiremock matches the official SDK HTTP shape, real
//! Infisical SDK client in-process, full webhook dispatch to flat files.
//!
//! For HTTP-only behavior (signatures, idempotency) with a mock provider, see `tests/webhook/`.

use axum::body::Body;
use bytes::Bytes;
use http::Request;
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::fixture as fx;
use crate::support;

async fn mount_sdk_login_ok(mock: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/auth/universal-auth/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accessToken": "stub-access-token",
            "expiresIn": 3600,
            "accessTokenMaxTTL": 7200,
            "tokenType": "Bearer"
        })))
        .mount(mock)
        .await;
}

async fn mount_sdk_list_ok(mock: &MockServer, secrets: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v3/secrets/raw"))
        .and(query_param("workspaceId", fx::STUB_WORKSPACE_ID))
        .and(query_param("environment", "prod"))
        .and(query_param("secretPath", "/papra"))
        .and(query_param("expandSecretReferences", "true"))
        .and(query_param("recursive", "true"))
        .and(query_param("include_imports", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "imports": [],
            "secrets": secrets
        })))
        .mount(mock)
        .await;
}

async fn post_signed_webhook(app: &axum::Router, body: &[u8]) -> axum::response::Response {
    let body = Bytes::copy_from_slice(body);
    let headers = fx::signed_headers(fx::WEBHOOK_SIGNING_SECRET, &body);
    let mut req_builder = Request::builder()
        .method("POST")
        .uri(format!("/webhooks/{}", fx::PROVIDER_NAME))
        .header("content-type", "application/json");
    for (name, value) in headers.iter() {
        req_builder = req_builder.header(name, value);
    }
    let req = req_builder.body(Body::from(body)).unwrap();
    app.clone().oneshot(req).await.unwrap()
}

#[tokio::test]
async fn webhook_fetches_and_writes_flat_files() {
    let mock_server = MockServer::start().await;
    mount_sdk_login_ok(&mock_server).await;
    mount_sdk_list_ok(
        &mock_server,
        serde_json::json!([
            {
                "_id": "stub-id",
                "workspace": fx::STUB_WORKSPACE_ID,
                "version": 1,
                "type": "shared",
                "environment": "prod",
                "secretKey": "AUTH_SECRET",
                "secretValue": "value-from-stub",
                "secretComment": ""
            }
        ]),
    )
    .await;

    let temp = tempdir().unwrap();
    let db = temp.path().join("providers_stub.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();

    let cfg = fx::papra_app_config(mock_server.uri(), db, out.clone());
    let engine = support::engine_with_config_providers(cfg).await.unwrap();
    let app = support::app_with_engine(engine);

    let body = r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#;
    let response = post_signed_webhook(&app, body.as_bytes()).await;

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

#[tokio::test]
async fn webhook_applies_include_keys_filter() {
    let mock_server = MockServer::start().await;
    mount_sdk_login_ok(&mock_server).await;
    mount_sdk_list_ok(
        &mock_server,
        serde_json::json!([
            {
                "_id": "a",
                "workspace": fx::STUB_WORKSPACE_ID,
                "version": 1,
                "type": "shared",
                "environment": "prod",
                "secretKey": "KEEP_ME",
                "secretValue": "keep-value",
                "secretComment": ""
            },
            {
                "_id": "b",
                "workspace": fx::STUB_WORKSPACE_ID,
                "version": 1,
                "type": "shared",
                "environment": "prod",
                "secretKey": "DROP_ME",
                "secretValue": "drop-value",
                "secretComment": ""
            }
        ]),
    )
    .await;

    let temp = tempdir().unwrap();
    let db = temp.path().join("providers_include.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();

    let cfg = fx::papra_app_config_with_include_keys(
        mock_server.uri(),
        db,
        out.clone(),
        vec!["KEEP_ME".to_string()],
    );
    let engine = support::engine_with_config_providers(cfg).await.unwrap();
    let app = support::app_with_engine(engine);

    let body = r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#;
    let response = post_signed_webhook(&app, body.as_bytes()).await;
    assert_eq!(response.status(), http::StatusCode::OK);

    assert!(out.join("keep_me").is_file());
    assert_eq!(
        std::fs::read_to_string(out.join("keep_me")).unwrap(),
        "keep-value"
    );
    assert!(!out.join("drop_me").exists());
}

#[tokio::test]
async fn webhook_secret_list_http_error_surfaces_as_bad_request() {
    let mock_server = MockServer::start().await;
    mount_sdk_login_ok(&mock_server).await;

    Mock::given(method("GET"))
        .and(path("/api/v3/secrets/raw"))
        .and(query_param("workspaceId", fx::STUB_WORKSPACE_ID))
        .and(query_param("environment", "prod"))
        .and(query_param("secretPath", "/papra"))
        .and(query_param("expandSecretReferences", "true"))
        .and(query_param("recursive", "true"))
        .and(query_param("include_imports", "true"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "reqId": "test-req",
            "message": "Invalid workspace or path"
        })))
        .mount(&mock_server)
        .await;

    let temp = tempdir().unwrap();
    let db = temp.path().join("providers_err.db");
    let out = temp.path().join("secrets");
    std::fs::create_dir_all(&out).unwrap();

    let cfg = fx::papra_app_config(mock_server.uri(), db, out);
    let engine = support::engine_with_config_providers(cfg).await.unwrap();
    let app = support::app_with_engine(engine);

    let body = r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#;
    let response = post_signed_webhook(&app, body.as_bytes()).await;

    assert_eq!(response.status(), http::StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let err = v["error"].as_str().expect("error string");
    assert!(
        err.contains("400") || err.contains("Invalid workspace") || err.contains("Infisical"),
        "unexpected error payload: {err}"
    );
}
