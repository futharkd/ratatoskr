use mimir::config::PlaceholderOverride;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SecurityProfileConfig {
    #[serde(default)]
    pub allow_env_vars: bool,
    #[serde(default)]
    pub require_signature: bool,
    #[serde(default)]
    pub replay_tolerance_seconds: Option<i64>,
    #[serde(default)]
    pub placeholders: PlaceholderOverride,
}
