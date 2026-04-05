use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::providers::{BuildProviderFromConfig, ProviderClient};

use super::defaults::DefaultsConfig;

/// Provider-specific payload types live under `crate::providers::<name>/config.rs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderKind {
    Infisical(crate::providers::infisical::InfisicalProviderConfig),
}

impl ProviderKind {
    pub fn into_client(
        self,
        name: String,
        defaults: &DefaultsConfig,
    ) -> anyhow::Result<Arc<dyn ProviderClient>> {
        match self {
            ProviderKind::Infisical(cfg) => cfg.into_provider_client(name, defaults),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(flatten)]
    pub kind: ProviderKind,
}
