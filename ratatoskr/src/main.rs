mod config;
mod dispatch;
mod http;
mod orchestration;
mod placeholders;
mod providers;
mod render;
mod storage;
mod verify;

use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{Router, routing::get};
use config::AppConfig;
use dispatch::DispatchEngine;
use providers::{ProviderClient, infisical::InfisicalProvider};
use storage::build_idempotency_store;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<DispatchEngine>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config_path = env::args()
        .nth(1)
        .or_else(|| env::var("RATATOSKR_CONFIG").ok())
        .unwrap_or_else(|| "examples/ratatoskr.example.toml".to_string());

    let config = AppConfig::load(&config_path)
        .with_context(|| format!("failed loading config at {config_path}"))?;
    let providers = build_provider_map(&config);
    let store = build_idempotency_store(&config.storage).await?;
    let engine = DispatchEngine::new(config.clone(), providers, store);

    let app_state = AppState {
        engine: Arc::new(engine),
    };

    let app = Router::new()
        .route("/healthz", get(http::healthz))
        .route(
            "/webhooks/:provider",
            axum::routing::post(http::webhook::handle),
        )
        .with_state(app_state);

    let addr: SocketAddr = config.server.listen_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("ratatoskr listening on {}", config.server.listen_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

fn build_provider_map(config: &AppConfig) -> HashMap<String, Arc<dyn ProviderClient>> {
    let mut map: HashMap<String, Arc<dyn ProviderClient>> = HashMap::new();
    for provider in &config.providers {
        let config::ProviderKind::Infisical(infisical) = &provider.kind;
        let client = InfisicalProvider::new(
            provider.name.clone(),
            infisical.clone(),
            config.defaults.max_retries,
            config.defaults.retry_backoff_millis,
            config.defaults.http_timeout_seconds,
        );
        map.insert(provider.name.clone(), Arc::new(client));
    }
    map
}
