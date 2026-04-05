use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub replay_tolerance_seconds: i64,
    #[serde(default)]
    pub http_timeout_seconds: u64,
    #[serde(default)]
    pub max_retries: usize,
    #[serde(default)]
    pub retry_backoff_millis: u64,
}
