use std::{collections::BTreeMap, sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use infisical::secrets::ListSecretsRequest;
use infisical::{AuthMethod, Client, InfisicalError};
use reqwest::StatusCode;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::config::{DefaultsConfig, SecretSelector};
use crate::providers::{BuildProviderFromConfig, ProviderClient, SecretFetchRequest, SecretMap};

use super::InfisicalProviderConfig;
use mimir::placeholders::{PlaceholderPolicy, resolve_placeholders};

fn should_clear_sdk_client(err: &InfisicalError) -> bool {
    matches!(
        err,
        InfisicalError::NotAuthenticated
            | InfisicalError::InvalidAuthMethod
            | InfisicalError::InvalidAuthHeaderValue(_)
    ) || matches!(
        err,
        InfisicalError::HttpError { status, .. }
            if *status == StatusCode::UNAUTHORIZED || *status == StatusCode::FORBIDDEN
    )
}

/// Normalize folder path for the Infisical SDK (leading slash; empty becomes `/`).
fn list_secrets_path(secret_path: &str) -> String {
    let t = secret_path.trim();
    if t.is_empty() {
        return "/".to_string();
    }
    if t.starts_with('/') {
        t.to_string()
    } else {
        format!("/{t}")
    }
}

pub struct InfisicalProvider {
    name: String,
    config: InfisicalProviderConfig,
    max_retries: usize,
    retry_backoff_millis: u64,
    http_timeout_seconds: u64,
    sdk: Mutex<Option<Client>>,
}

impl InfisicalProvider {
    pub fn new(
        name: String,
        config: InfisicalProviderConfig,
        max_retries: usize,
        retry_backoff_millis: u64,
        http_timeout_seconds: u64,
    ) -> Self {
        Self {
            name,
            config,
            max_retries,
            retry_backoff_millis,
            http_timeout_seconds,
            sdk: Mutex::new(None),
        }
    }

    fn resolve_provider_secret(&self, raw_value: &str) -> anyhow::Result<String> {
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: true,
        };
        resolve_placeholders(raw_value, policy)
            .with_context(|| "failed resolving provider placeholder values")
    }

    async fn ensure_logged_in_client(&self) -> anyhow::Result<()> {
        let mut guard = self.sdk.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        let base = self.config.api_base_url.trim_end_matches('/').to_string();
        let timeout = Duration::from_secs(self.http_timeout_seconds.max(1));

        let mut client = Client::builder()
            .base_url(base)
            .request_timeout(timeout)
            .user_agent("ratatoskr")
            .build()
            .await
            .context("failed building Infisical SDK client")?;

        let client_id = self.resolve_provider_secret(&self.config.client_id)?;
        let client_secret = self.resolve_provider_secret(&self.config.client_secret)?;
        let auth = AuthMethod::new_universal_auth(client_id, client_secret);
        client
            .login(auth)
            .await
            .context("Infisical universal-auth login failed")?;

        *guard = Some(client);
        Ok(())
    }

    async fn list_secrets_with_sdk(&self, selector: &SecretSelector) -> anyhow::Result<SecretMap> {
        self.ensure_logged_in_client().await?;

        let project_id = self.resolve_provider_secret(&self.config.project_id)?;
        let path = list_secrets_path(&selector.secret_path);

        let request = ListSecretsRequest::builder(project_id, &selector.environment)
            .path(path)
            .recursive(true)
            .build();

        let guard = self.sdk.lock().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow!("Infisical SDK client missing after login"))?;

        let listed = client
            .secrets()
            .list(request)
            .await
            .context("Infisical list secrets failed")?;

        let mut out = BTreeMap::new();
        for item in listed {
            if !selector.include_keys.is_empty()
                && !selector.include_keys.contains(&item.secret_key)
            {
                continue;
            }
            out.insert(item.secret_key, item.secret_value);
        }
        Ok(out)
    }

    async fn fetch_secrets_inner(&self, selector: &SecretSelector) -> anyhow::Result<SecretMap> {
        match self.list_secrets_with_sdk(selector).await {
            Ok(m) => Ok(m),
            Err(e) => {
                if e.root_cause()
                    .downcast_ref::<InfisicalError>()
                    .is_some_and(should_clear_sdk_client)
                {
                    let mut guard = self.sdk.lock().await;
                    *guard = None;
                }
                Err(e)
            }
        }
    }
}

#[async_trait]
impl ProviderClient for InfisicalProvider {
    async fn fetch_secrets(&self, request: SecretFetchRequest) -> anyhow::Result<SecretMap> {
        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 0..self.max_retries {
            match self.fetch_secrets_inner(&request.selector).await {
                Ok(secrets) => return Ok(secrets),
                Err(err) => last_error = Some(err),
            }
            let backoff_ms = self
                .retry_backoff_millis
                .saturating_mul((attempt + 1) as u64);
            sleep(Duration::from_millis(backoff_ms)).await;
        }
        Err(last_error.unwrap_or_else(|| anyhow!("secret fetch failed with unknown error")))
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    fn webhook_secret(&self) -> anyhow::Result<String> {
        self.resolve_provider_secret(&self.config.webhook_secret)
    }
}

impl BuildProviderFromConfig for InfisicalProviderConfig {
    fn into_provider_client(
        self,
        name: String,
        defaults: &DefaultsConfig,
    ) -> anyhow::Result<Arc<dyn ProviderClient>> {
        Ok(Arc::new(InfisicalProvider::new(
            name,
            self,
            defaults.max_retries,
            defaults.retry_backoff_millis,
            defaults.http_timeout_seconds,
        )))
    }
}
