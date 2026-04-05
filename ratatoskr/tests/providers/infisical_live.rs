//! Opt-in tests against a real Infisical project. See `DEVELOPMENT.md` in this crate.
//! Run with: `cargo test -p ratatoskr --test providers -- --ignored --nocapture`

use std::sync::Arc;

use ratatoskr::{
    config::{DefaultsConfig, ProviderKind, SecretSelector},
    providers::{ProviderClient, SecretFetchRequest},
};

use crate::support;

#[tokio::test]
#[ignore = "requires Infisical credentials in env; see DEVELOPMENT.md"]
async fn infisical_live_fetch_secrets() {
    let base = std::env::var("RATATOSKR_INFISICAL_API_BASE_URL")
        .expect("RATATOSKR_INFISICAL_API_BASE_URL (e.g. https://app.infisical.com)");
    let client_id =
        std::env::var("RATATOSKR_INFISICAL_CLIENT_ID").expect("RATATOSKR_INFISICAL_CLIENT_ID");
    let client_secret = std::env::var("RATATOSKR_INFISICAL_CLIENT_SECRET")
        .expect("RATATOSKR_INFISICAL_CLIENT_SECRET");
    let environment = std::env::var("RATATOSKR_INFISICAL_ENVIRONMENT")
        .expect("RATATOSKR_INFISICAL_ENVIRONMENT (e.g. dev)");
    let secret_path = std::env::var("RATATOSKR_INFISICAL_SECRET_PATH")
        .expect("RATATOSKR_INFISICAL_SECRET_PATH (e.g. /my-app)");

    let expect_key = std::env::var("RATATOSKR_INFISICAL_EXPECT_KEY")
        .unwrap_or_else(|_| "AUTH_SECRET".to_string());

    let mut provider_cfg = support::sample_infisical_provider_config(base);
    provider_cfg.client_id = client_id;
    provider_cfg.client_secret = client_secret;
    if let Ok(s) = std::env::var("RATATOSKR_INFISICAL_WEBHOOK_SECRET") {
        provider_cfg.webhook_secret = s;
    }

    let defaults = DefaultsConfig {
        replay_tolerance_seconds: 300,
        http_timeout_seconds: 30,
        max_retries: 3,
        retry_backoff_millis: 300,
    };

    let client: Arc<dyn ProviderClient> = ProviderKind::Infisical(provider_cfg)
        .into_client("live_test".to_string(), &defaults)
        .expect("provider client");

    let secrets = client
        .fetch_secrets(SecretFetchRequest {
            selector: SecretSelector {
                environment,
                secret_path,
                include_keys: Vec::new(),
            },
        })
        .await
        .expect("fetch secrets from Infisical");

    assert!(
        secrets.contains_key(&expect_key),
        "expected secret key {expect_key:?} in response; keys: {:?}",
        secrets.keys().collect::<Vec<_>>()
    );
}
