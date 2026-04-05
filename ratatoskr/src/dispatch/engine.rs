use std::{collections::HashMap, env, sync::Arc};

use anyhow::{Context, anyhow};
use bytes::Bytes;
use http::HeaderMap;

use crate::{
    config::{AppConfig, PlaceholderPolicyOverride, ProviderConfig, ProviderKind, ServiceConfig},
    orchestration::LifecycleExecutor,
    providers::{ProviderClient, SecretFetchRequest},
    render::{PlaceholderPolicy, render_and_write},
    storage::IdempotencyStore,
    verify::verify_infisical_signature,
};

use super::{
    idempotency::build_event_key,
    matching::match_services,
    types::{DispatchResult, WebhookPayload},
};

#[derive(Clone)]
pub struct DispatchEngine {
    config: AppConfig,
    providers: HashMap<String, Arc<dyn ProviderClient>>,
    idempotency: Arc<dyn IdempotencyStore>,
    lifecycle: LifecycleExecutor,
}

impl DispatchEngine {
    pub fn new(
        config: AppConfig,
        providers: HashMap<String, Arc<dyn ProviderClient>>,
        idempotency: Arc<dyn IdempotencyStore>,
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

        let matched_services = match_services(&self.config.services, provider_name, &payload);
        let mut applied = 0usize;
        for service in &matched_services {
            let request = SecretFetchRequest {
                selector: service.secret_selector.clone(),
            };
            let secrets = provider
                .fetch_secrets(request)
                .await
                .with_context(|| format!("failed fetching secrets for service {}", service.name))?;
            let placeholder_policy = self.effective_placeholder_policy(service);
            render_and_write(&service.output, &secrets, placeholder_policy)
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
        provider_cfg: &ProviderConfig,
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

    fn effective_placeholder_policy(&self, service: &ServiceConfig) -> PlaceholderPolicy {
        let profile = self.config.security_profiles.get(&service.security_profile);
        let base = PlaceholderPolicy {
            allow_env_placeholders: profile.map(|p| p.allow_env_placeholders).unwrap_or(false),
            allow_file_placeholders: profile.map(|p| p.allow_file_placeholders).unwrap_or(false),
        };
        apply_placeholder_override(base, service.placeholder_policy_override.as_ref())
    }
}

fn apply_placeholder_override(
    base: PlaceholderPolicy,
    override_cfg: Option<&PlaceholderPolicyOverride>,
) -> PlaceholderPolicy {
    if let Some(override_cfg) = override_cfg {
        PlaceholderPolicy {
            allow_env_placeholders: override_cfg
                .allow_env_placeholders
                .unwrap_or(base.allow_env_placeholders),
            allow_file_placeholders: override_cfg
                .allow_file_placeholders
                .unwrap_or(base.allow_file_placeholders),
        }
    } else {
        base
    }
}
