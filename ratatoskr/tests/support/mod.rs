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
use ratatoskr::{
    AppState, DispatchEngine,
    config::AppConfig,
    providers::{ProviderClient, SecretFetchRequest, SecretMap},
    router,
    storage::{IdempotencyStore, sqlite::SqliteIdempotencyStore},
};

#[derive(Clone)]
pub struct MockProvider {
    pub calls: Arc<AtomicUsize>,
    pub name: String,
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
        &self.name
    }

    fn webhook_secret(&self) -> anyhow::Result<String> {
        // Keep in sync with `tests/providers/infisical/fixture.rs` (`WEBHOOK_SIGNING_SECRET`).
        Ok("top-secret".to_string())
    }
}

/// Config with no providers/services (sufficient for `/healthz`).
pub fn empty_app_config(sqlite_path: PathBuf) -> AppConfig {
    use ratatoskr::config::{
        ConfigIncludes, DefaultsConfig, MimirConfig, ServerConfig, StorageBackend, StorageConfig,
    };
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

/// Build [`DispatchEngine`] with real provider clients wired from `cfg.providers`.
pub async fn engine_with_config_providers(cfg: AppConfig) -> anyhow::Result<Arc<DispatchEngine>> {
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    for provider in &cfg.providers {
        let client = provider
            .kind
            .clone()
            .into_client(provider.name.clone(), &cfg.defaults)?;
        providers.insert(provider.name.clone(), client);
    }
    let store: Arc<dyn IdempotencyStore> =
        Arc::new(SqliteIdempotencyStore::new(&cfg.storage.sqlite_path).await?);
    Ok(Arc::new(DispatchEngine::new(cfg, providers, store)))
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

/// Swaps in [`MockProvider`] for the first provider named in `cfg.providers`.
pub async fn engine_with_webhook_mock(
    cfg: AppConfig,
    calls: Arc<AtomicUsize>,
) -> Arc<DispatchEngine> {
    let provider_name = cfg
        .providers
        .first()
        .map(|p| p.name.clone())
        .expect("app config must declare at least one provider");
    let provider = Arc::new(MockProvider {
        calls,
        name: provider_name.clone(),
    });
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    providers.insert(provider_name, provider);
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
