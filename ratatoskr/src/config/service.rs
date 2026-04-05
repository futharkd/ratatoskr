use mimir::config::PlaceholderOverride;
use serde::{Deserialize, Serialize};

use super::{lifecycle::LifecycleAction, output::OutputConfig};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub name: String,
    pub provider: String,
    pub secret_selector: SecretSelector,
    pub output: OutputConfig,
    #[serde(default)]
    pub lifecycle: LifecycleAction,
    #[serde(default)]
    pub security_profile: String,
    #[serde(default)]
    pub placeholder_policy_override: Option<PlaceholderOverride>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecretSelector {
    pub environment: String,
    #[serde(default = "default_secret_path")]
    pub secret_path: String,
    #[serde(default)]
    pub include_keys: Vec<String>,
}

fn default_secret_path() -> String {
    "/".to_string()
}
