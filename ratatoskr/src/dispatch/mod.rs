use std::{collections::HashMap, env, sync::Arc};

use anyhow::{anyhow, Context};
use bytes::Bytes;
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::{
    config::{AppConfig, ProviderKind, ServiceConfig},
    orchestration::LifecycleExecutor,
    providers::{ProviderClient, SecretFetchRequest},
    render::render_and_write,
    storage::SqliteIdempotencyStore,
    verify::verify_infisical_signature,
};

#[derive(Debug, Clone, Serialize)]
pub struct DispatchResult {
    pub provider: String,
    pub matched_services: usize,
    pub applied_services: usize,
    pub skipped_duplicate: bool,
}

#[derive(Clone)]
pub struct DispatchEngine {
    config: AppConfig,
    providers: HashMap<String, Arc<dyn ProviderClient>>,
    idempotency: SqliteIdempotencyStore,
    lifecycle: LifecycleExecutor,
}

impl DispatchEngine {
    pub fn new(
        config: AppConfig,
        providers: HashMap<String, Arc<dyn ProviderClient>>,
        idempotency: SqliteIdempotencyStore,
    ) -> Self {
        Self {
            config,
            providers,
            idempotency,
            lifecycle: LifecycleExecutor::new(),
        }
    }

    pub async fn process_webhook(
        &self,
        provider_name: &str,
        headers: &HeaderMap,
        body: &Bytes,
    ) -> anyhow::Result<DispatchResult> {
        let provider_cfg = self
            .config
            .providers
            .iter()
            .find(|p| p.name == provider_name)
            .ok_or_else(|| anyhow!("unknown provider {provider_name}"))?;
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("provider runtime missing for {provider_name}"))?;
        self.verify_payload(provider_cfg, provider.as_ref(), headers, body)?;

        let payload: WebhookPayload =
            serde_json::from_slice(body).context("invalid webhook JSON payload")?;
        let event_key = build_event_key(provider_name, headers, body);
        if !self.idempotency.mark_if_new(&event_key).await? {
            return Ok(DispatchResult {
                provider: provider_name.to_string(),
                matched_services: 0,
                applied_services: 0,
                skipped_duplicate: true,
            });
        }

        let matched_services = self.match_services(provider_name, &payload);
        let mut applied = 0usize;
        for service in matched_services.iter() {
            let request = SecretFetchRequest {
                selector: service.secret_selector.clone(),
            };
            let secrets = provider
                .fetch_secrets(request)
                .await
                .with_context(|| format!("failed fetching secrets for service {}", service.name))?;
            render_and_write(&service.output, &secrets)
                .with_context(|| format!("failed rendering output for service {}", service.name))?;
            self.lifecycle
                .execute(&service.lifecycle)
                .await
                .with_context(|| {
                    format!(
                        "failed executing lifecycle action for service {}",
                        service.name
                    )
                })?;
            applied += 1;
        }

        Ok(DispatchResult {
            provider: provider_name.to_string(),
            matched_services: matched_services.len(),
            applied_services: applied,
            skipped_duplicate: false,
        })
    }

    fn verify_payload(
        &self,
        provider_cfg: &crate::config::ProviderConfig,
        provider: &dyn ProviderClient,
        headers: &HeaderMap,
        body: &Bytes,
    ) -> anyhow::Result<()> {
        let secret = env::var(provider.webhook_secret_env_var())
            .with_context(|| format!("missing env var {}", provider.webhook_secret_env_var()))?;

        match &provider_cfg.kind {
            ProviderKind::Infisical(_) => {
                verify_infisical_signature(
                    headers,
                    body,
                    &secret,
                    self.config.defaults.replay_tolerance_seconds,
                )?;
            }
        }
        Ok(())
    }

    fn match_services(&self, provider_name: &str, payload: &WebhookPayload) -> Vec<ServiceConfig> {
        let mut out = Vec::new();
        for service in &self.config.services {
            if service.provider != provider_name {
                continue;
            }
            if let Some(event_env) = &payload.environment {
                if service.secret_selector.environment != *event_env {
                    continue;
                }
            }
            if let Some(event_path) = &payload.secret_path {
                if service.secret_selector.secret_path != *event_path {
                    warn!(
                        service = service.name,
                        expected_path = service.secret_selector.secret_path,
                        event_path = event_path,
                        "service skipped due to secret path mismatch"
                    );
                    continue;
                }
            }
            out.push(service.clone());
        }
        out
    }
}

fn build_event_key(provider_name: &str, headers: &HeaderMap, body: &Bytes) -> String {
    let delivery_id = headers
        .get("x-infisical-delivery-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(provider_name.as_bytes());
    hasher.update(delivery_id.as_bytes());
    hasher.update(body);
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Deserialize)]
struct WebhookPayload {
    #[allow(dead_code)]
    event: Option<String>,
    #[serde(default)]
    environment: Option<String>,
    #[serde(rename = "secretPath", default)]
    secret_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use hmac::{Hmac, KeyInit, Mac};
    use http::{header::HeaderValue, HeaderMap};
    use sha2::Sha256;
    use tempfile::tempdir;

    use crate::{
        config::{
            AppConfig, DefaultsConfig, InfisicalProviderConfig, LifecycleAction, OutputConfig,
            ProviderConfig, ProviderKind, SecretSelector, ServerConfig, ServiceConfig,
            StorageConfig,
        },
        providers::{ProviderClient, SecretFetchRequest, SecretMap},
        storage::SqliteIdempotencyStore,
    };

    use super::DispatchEngine;

    type HmacSha256 = Hmac<Sha256>;

    #[derive(Clone)]
    struct MockProvider {
        calls: Arc<AtomicUsize>,
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

        fn webhook_secret_env_var(&self) -> &str {
            "TEST_WEBHOOK_SECRET"
        }
    }

    #[tokio::test]
    async fn deduplicates_duplicate_webhook_events() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("idempotency.db");
        let output_dir = temp.path().join("secrets");

        let cfg = AppConfig {
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
                sqlite_path: db_path.to_string_lossy().into_owned(),
            },
            providers: vec![ProviderConfig {
                name: "infisical_main".to_string(),
                kind: ProviderKind::Infisical(InfisicalProviderConfig {
                    api_base_url: "https://app.infisical.com".to_string(),
                    client_id_env: "X".to_string(),
                    client_secret_env: "Y".to_string(),
                    webhook_secret_env: "TEST_WEBHOOK_SECRET".to_string(),
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
            }],
            security_profiles: Default::default(),
        };

        std::env::set_var("TEST_WEBHOOK_SECRET", "top-secret");
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(MockProvider {
            calls: calls.clone(),
        });
        let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
        providers.insert("infisical_main".to_string(), provider);
        let store = SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap();
        let engine = DispatchEngine::new(cfg, providers, store);

        let body = Bytes::from(
            r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#,
        );
        let headers = signed_headers("top-secret", &body);

        let first = engine
            .process_webhook("infisical_main", &headers, &body)
            .await
            .unwrap();
        let second = engine
            .process_webhook("infisical_main", &headers, &body)
            .await
            .unwrap();

        assert_eq!(first.applied_services, 1);
        assert!(second.skipped_duplicate);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    fn signed_headers(secret: &str, body: &Bytes) -> HeaderMap {
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
}
