//! Infisical-only test fixtures: sample config, HMAC webhook headers matching production.
//!
//! Used by `infisical::stub` and by `tests/webhook/` (Infisical-shaped payloads and signatures).
#![allow(dead_code)] // `#[path]`-shared; each integration binary only uses a subset of helpers.

use std::path::PathBuf;

use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use http::{HeaderMap, header::HeaderValue};
use ratatoskr::config::{
    AppConfig, ConfigIncludes, DefaultsConfig, InfisicalProviderConfig, LifecycleAction,
    MimirConfig, OutputConfig, ProviderConfig, ProviderKind, SecretSelector, ServerConfig,
    ServiceConfig, StorageBackend, StorageConfig,
};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const PROVIDER_NAME: &str = "infisical_main";
pub const STUB_WORKSPACE_ID: &str = "stub-workspace-id";
pub const WEBHOOK_SIGNING_SECRET: &str = "top-secret";

/// Headers for an Infisical-signed webhook (`x-infisical-signature`, delivery id).
pub fn signed_headers(secret: &str, body: &Bytes) -> HeaderMap {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let payload = format!("{timestamp}.{}", String::from_utf8_lossy(body));
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-infisical-signature",
        HeaderValue::from_str(&format!("t={timestamp};signature={signature}")).unwrap(),
    );
    headers.insert(
        "x-infisical-delivery-id",
        HeaderValue::from_static("delivery-1"),
    );
    headers
}

pub fn stub_infisical_provider_config(api_base_url: impl Into<String>) -> InfisicalProviderConfig {
    InfisicalProviderConfig {
        api_base_url: api_base_url.into(),
        project_id: STUB_WORKSPACE_ID.to_string(),
        client_id: "x".to_string(),
        client_secret: "y".to_string(),
        webhook_secret: WEBHOOK_SIGNING_SECRET.to_string(),
    }
}

fn base_app_shell(sqlite_path: PathBuf) -> (ServerConfig, DefaultsConfig, StorageConfig) {
    (
        ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: sqlite_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
    )
}

/// One Infisical provider + one matching `papra` service; `api_base_url` is the stub or a dummy URL.
pub fn papra_app_config(
    api_base_url: impl Into<String>,
    sqlite_path: PathBuf,
    output_dir: PathBuf,
) -> AppConfig {
    let api_base_url = api_base_url.into();
    let (server, defaults, storage) = base_app_shell(sqlite_path);
    AppConfig {
        server,
        defaults,
        storage,
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![ProviderConfig {
            name: PROVIDER_NAME.to_string(),
            kind: ProviderKind::Infisical(stub_infisical_provider_config(api_base_url)),
        }],
        services: vec![ServiceConfig {
            name: "papra".to_string(),
            provider: PROVIDER_NAME.to_string(),
            secret_selector: SecretSelector {
                environment: "prod".to_string(),
                secret_path: "/papra".to_string(),
                include_keys: Vec::new(),
            },
            output: OutputConfig::FlatFiles {
                directory: output_dir.to_string_lossy().into_owned(),
                file_mode: None,
            },
            lifecycle: LifecycleAction::NoAction,
            security_profile: "strict".to_string(),
            placeholder_policy_override: None,
        }],
        security_profiles: Default::default(),
    }
}

/// Same as [`papra_app_config`] with a non-empty `include_keys` filter on the service.
pub fn papra_app_config_with_include_keys(
    api_base_url: impl Into<String>,
    sqlite_path: PathBuf,
    output_dir: PathBuf,
    include_keys: Vec<String>,
) -> AppConfig {
    let mut cfg = papra_app_config(api_base_url, sqlite_path, output_dir);
    if let Some(svc) = cfg.services.first_mut() {
        svc.secret_selector.include_keys = include_keys;
    }
    cfg
}

/// Config for webhook tests that replace the Infisical client with [`crate::support::MockProvider`].
/// Uses a placeholder API base URL; outbound HTTP is never called.
pub fn papra_app_config_for_mock_provider(sqlite_path: PathBuf, output_dir: PathBuf) -> AppConfig {
    papra_app_config("https://app.infisical.com", sqlite_path, output_dir)
}
