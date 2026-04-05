use std::collections::HashMap;

use mimir::config::{MimirConfig, PlaceholderOverride};
use serde::{Deserialize, Serialize};

use super::{
    defaults::DefaultsConfig, includes::ConfigIncludes, profile::SecurityProfileConfig,
    provider::ProviderConfig, server::ServerConfig, service::ServiceConfig, storage::StorageConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub includes: ConfigIncludes,
    #[serde(default)]
    pub mimir: MimirConfig,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    #[serde(default)]
    pub security_profiles: HashMap<String, SecurityProfileConfig>,
}

impl AppConfig {
    pub(crate) fn apply_defaults(&mut self) {
        if self.defaults.replay_tolerance_seconds == 0 {
            self.defaults.replay_tolerance_seconds = 300;
        }
        if self.defaults.http_timeout_seconds == 0 {
            self.defaults.http_timeout_seconds = 10;
        }
        if self.defaults.max_retries == 0 {
            self.defaults.max_retries = 3;
        }
        if self.defaults.retry_backoff_millis == 0 {
            self.defaults.retry_backoff_millis = 300;
        }
        self.mimir = self.mimir.clone().with_fallbacks(&default_mimir_config());
    }
}

pub(crate) fn default_mimir_config() -> MimirConfig {
    MimirConfig {
        placeholders: PlaceholderOverride {
            env: Some(false),
            file: Some(false),
        },
    }
}
