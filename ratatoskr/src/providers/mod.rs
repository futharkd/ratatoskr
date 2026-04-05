pub mod infisical;

use std::collections::BTreeMap;

use async_trait::async_trait;

use mimir::config::SecretSelector;

#[derive(Debug, Clone)]
pub struct SecretFetchRequest {
    pub selector: SecretSelector,
}

pub type SecretMap = BTreeMap<String, String>;

#[async_trait]
pub trait ProviderClient: Send + Sync {
    async fn fetch_secrets(&self, request: SecretFetchRequest) -> anyhow::Result<SecretMap>;
    fn provider_name(&self) -> &str;
    fn webhook_secret(&self) -> anyhow::Result<String>;
}
