use std::{collections::BTreeMap, env, time::Duration};

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::time::sleep;

use crate::{
    config::{InfisicalProviderConfig, SecretSelector},
    providers::{ProviderClient, SecretFetchRequest, SecretMap},
};

#[derive(Clone)]
pub struct InfisicalProvider {
    name: String,
    config: InfisicalProviderConfig,
    client: Client,
    max_retries: usize,
    retry_backoff_millis: u64,
}

impl InfisicalProvider {
    pub fn new(
        name: String,
        config: InfisicalProviderConfig,
        max_retries: usize,
        retry_backoff_millis: u64,
        http_timeout_seconds: u64,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(http_timeout_seconds))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            name,
            config,
            client,
            max_retries,
            retry_backoff_millis,
        }
    }

    async fn login(&self) -> anyhow::Result<String> {
        let client_id = env::var(&self.config.client_id_env)
            .with_context(|| format!("missing env var {}", self.config.client_id_env))?;
        let client_secret = env::var(&self.config.client_secret_env)
            .with_context(|| format!("missing env var {}", self.config.client_secret_env))?;

        let url = format!(
            "{}{}",
            self.config.api_base_url.trim_end_matches('/'),
            self.config.login_path
        );
        let body = serde_json::json!({
            "clientId": client_id,
            "clientSecret": client_secret,
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .context("infisical login request failed")?;
        if !response.status().is_success() {
            return Err(anyhow!("infisical login failed with {}", response.status()));
        }
        let payload: LoginResponse = response
            .json()
            .await
            .context("invalid infisical login response")?;
        Ok(payload.access_token)
    }

    async fn fetch_with_token(
        &self,
        access_token: &str,
        selector: &SecretSelector,
    ) -> anyhow::Result<SecretMap> {
        let url = format!(
            "{}{}",
            self.config.api_base_url.trim_end_matches('/'),
            self.config.secrets_path
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .query(&[
                ("environment", selector.environment.as_str()),
                ("secretPath", selector.secret_path.as_str()),
            ])
            .send()
            .await
            .context("infisical secret fetch failed")?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "infisical secret fetch failed with {}",
                response.status()
            ));
        }

        let payload: Value = response
            .json()
            .await
            .context("invalid infisical secret response payload")?;
        let secret_items = parse_secret_items(payload)?;

        let mut out = BTreeMap::new();
        for item in secret_items {
            if !selector.include_keys.is_empty()
                && !selector.include_keys.contains(&item.secret_key)
            {
                continue;
            }
            out.insert(item.secret_key, item.secret_value.unwrap_or_default());
        }
        Ok(out)
    }
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct SecretItem {
    #[serde(rename = "secretKey")]
    secret_key: String,
    #[serde(rename = "secretValue")]
    secret_value: Option<String>,
}

fn parse_secret_items(payload: Value) -> anyhow::Result<Vec<SecretItem>> {
    if let Ok(parsed) = serde_json::from_value::<SecretEnvelope>(payload.clone()) {
        return Ok(parsed.secrets);
    }

    let secrets = payload
        .get("secrets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("missing `secrets` array in provider response"))?;
    let mut items = Vec::new();
    for secret in secrets {
        let key = secret
            .get("secretKey")
            .or_else(|| secret.get("key"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("secret entry missing key"))?
            .to_string();
        let value = secret
            .get("secretValue")
            .or_else(|| secret.get("value"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        items.push(SecretItem {
            secret_key: key,
            secret_value: value,
        });
    }
    Ok(items)
}

#[derive(Debug, Deserialize)]
struct SecretEnvelope {
    #[serde(default)]
    secrets: Vec<SecretItem>,
}

#[async_trait]
impl ProviderClient for InfisicalProvider {
    async fn fetch_secrets(&self, request: SecretFetchRequest) -> anyhow::Result<SecretMap> {
        let mut last_error: Option<anyhow::Error> = None;
        for attempt in 0..self.max_retries {
            match self.login().await {
                Ok(token) => match self.fetch_with_token(&token, &request.selector).await {
                    Ok(secrets) => return Ok(secrets),
                    Err(err) => last_error = Some(err),
                },
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

    fn webhook_secret_env_var(&self) -> &str {
        &self.config.webhook_secret_env
    }
}
