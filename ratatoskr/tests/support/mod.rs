//! Shared helpers for HTTP integration tests (separate crate from `ratatoskr`).
#![allow(dead_code)]

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use http::{HeaderMap, header::HeaderValue};
use ratatoskr::{
    AppState, DispatchEngine,
    config::{
        AppConfig, ConfigIncludes, DefaultsConfig, InfisicalProviderConfig, LifecycleAction,
        MimirConfig, OutputConfig, ProviderConfig, ProviderKind, SecretSelector, ServerConfig,
        ServiceConfig, StorageBackend, StorageConfig,
    },
    providers::{ProviderClient, SecretFetchRequest, SecretMap},
    router,
    storage::{IdempotencyStore, sqlite::SqliteIdempotencyStore},
};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct MockProvider {
    pub calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ProviderClient for MockProvider {
    async fn fetch_secrets(&self, _request: SecretFetchRequest) -> anyhow::Result<SecretMap> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(BTreeMap::from([(
            "AUTH_SECRET".to_string(),
            "value-1".to_string(),
        )]))
    }

    fn provider_name(&self) -> &str {
        "infisical_main"
    }

    fn webhook_secret(&self) -> anyhow::Result<String> {
        Ok("top-secret".to_string())
    }
}

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

/// Config with no providers/services (sufficient for `/healthz`).
pub fn empty_app_config(sqlite_path: PathBuf) -> AppConfig {
    AppConfig {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        defaults: DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        storage: StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: sqlite_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![],
        services: vec![],
        security_profiles: Default::default(),
    }
}

/// One Infisical provider in config (for signature verification) + one matching service.
pub fn webhook_sample_app_config(sqlite_path: PathBuf, output_dir: PathBuf) -> AppConfig {
    AppConfig {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        defaults: DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        storage: StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: sqlite_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![ProviderConfig {
            name: "infisical_main".to_string(),
            kind: ProviderKind::Infisical(InfisicalProviderConfig {
                api_base_url: "https://app.infisical.com".to_string(),
                client_id: "x".to_string(),
                client_secret: "y".to_string(),
                webhook_secret: "top-secret".to_string(),
                login_path: "/api/v1/auth/universal-auth/login".to_string(),
                secrets_path: "/api/v3/secrets/raw".to_string(),
            }),
        }],
        services: vec![ServiceConfig {
            name: "papra".to_string(),
            provider: "infisical_main".to_string(),
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

pub async fn engine_empty(sqlite_path: PathBuf) -> Arc<DispatchEngine> {
    let cfg = empty_app_config(sqlite_path);
    let store: Arc<dyn IdempotencyStore> = Arc::new(
        SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap(),
    );
    Arc::new(DispatchEngine::new(cfg, HashMap::new(), store))
}

pub async fn engine_with_webhook_mock(
    cfg: AppConfig,
    calls: Arc<AtomicUsize>,
) -> Arc<DispatchEngine> {
    let provider = Arc::new(MockProvider {
        calls: calls.clone(),
    });
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    providers.insert("infisical_main".to_string(), provider);
    let store: Arc<dyn IdempotencyStore> = Arc::new(
        SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap(),
    );
    Arc::new(DispatchEngine::new(cfg, providers, store))
}

pub fn app_with_engine(engine: Arc<DispatchEngine>) -> axum::Router {
    router(AppState { engine })
}
