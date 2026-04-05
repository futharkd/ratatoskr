pub mod infisical;

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::config::{DefaultsConfig, SecretSelector};

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

/// Implemented by each provider’s config struct so wiring stays next to the provider implementation.
pub trait BuildProviderFromConfig {
    fn into_provider_client(
        self,
        name: String,
        defaults: &DefaultsConfig,
    ) -> anyhow::Result<Arc<dyn ProviderClient>>;
}
