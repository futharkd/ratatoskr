//! Ratatoskr library: webhook worker, config, dispatch, and HTTP surface.
//! The binary in `main.rs` is a thin wrapper around [`run`].

pub mod config;
pub mod dispatch;
pub mod http;
pub mod orchestration;
pub mod providers;
pub mod render;
pub mod storage;
pub mod verify;

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{Router, routing::get};
use tokio::net::TcpListener;
use tracing::{info, warn};

pub use dispatch::DispatchEngine;

/// Shared Axum state for HTTP handlers.
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<DispatchEngine>,
}

/// HTTP application: `/healthz` and `POST /webhooks/{provider}`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(http::healthz))
        .route(
            "/webhooks/{provider}",
            axum::routing::post(http::webhook::handle),
        )
        .with_state(state)
}

pub fn init_tracing() {
    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

/// Wait for SIGINT (Ctrl+C) or, on Unix, SIGTERM (`docker stop`, Kubernetes, systemd).
async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(e) => warn!(error = %e, "failed to install Ctrl+C handler"),
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

/// Load config from `config_path`, bind [`AppConfig::server`](config::AppConfig), and serve until shutdown.
pub async fn run(config_path: &str) -> anyhow::Result<()> {
    let config = config::AppConfig::load(config_path)
        .with_context(|| format!("failed loading config at {config_path}"))?;
    let providers = build_provider_map(&config).context("failed building provider clients")?;
    let store = storage::build_idempotency_store(&config.storage).await?;
    let engine = DispatchEngine::new(config.clone(), providers, store);

    let app_state = AppState {
        engine: Arc::new(engine),
    };

    let app = router(app_state);

    let addr: SocketAddr = config.server.listen_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("ratatoskr listening on {}", config.server.listen_addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            shutdown_signal().await;
            info!("shutdown signal received, draining open connections");
        })
        .await?;
    Ok(())
}

fn build_provider_map(
    config: &config::AppConfig,
) -> anyhow::Result<HashMap<String, Arc<dyn providers::ProviderClient>>> {
    let mut map: HashMap<String, Arc<dyn providers::ProviderClient>> = HashMap::new();
    for provider in &config.providers {
        let client = provider
            .kind
            .clone()
            .into_client(provider.name.clone(), &config.defaults)?;
        map.insert(provider.name.clone(), client);
    }
    Ok(map)
}
