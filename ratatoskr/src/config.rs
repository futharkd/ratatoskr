use std::{collections::HashMap, fs, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    pub storage: StorageConfig,
    pub providers: Vec<ProviderConfig>,
    pub services: Vec<ServiceConfig>,
    #[serde(default)]
    pub security_profiles: HashMap<String, SecurityProfileConfig>,
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("unable to read config from {}", path.display()))?;
        let mut cfg: AppConfig = toml::from_str(&content)
            .with_context(|| format!("invalid TOML config at {}", path.display()))?;
        cfg.apply_defaults();
        Ok(cfg)
    }

    fn apply_defaults(&mut self) {
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
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub sqlite_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(flatten)]
    pub kind: ProviderKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderKind {
    Infisical(InfisicalProviderConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfisicalProviderConfig {
    #[serde(default = "default_infisical_base_url")]
    pub api_base_url: String,
    pub client_id_env: String,
    pub client_secret_env: String,
    pub webhook_secret_env: String,
    #[serde(default = "default_login_path")]
    pub login_path: String,
    #[serde(default = "default_secrets_path")]
    pub secrets_path: String,
}

fn default_infisical_base_url() -> String {
    "https://app.infisical.com".to_string()
}

fn default_login_path() -> String {
    "/api/v1/auth/universal-auth/login".to_string()
}

fn default_secrets_path() -> String {
    "/api/v3/secrets/raw".to_string()
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OutputConfig {
    FlatFiles {
        directory: String,
        #[serde(default)]
        file_mode: Option<u32>,
    },
    TemplatedYaml {
        file_path: String,
        template: String,
        #[serde(default)]
        file_mode: Option<u32>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LifecycleAction {
    #[default]
    NoAction,
    ReloadCaddy {
        admin_url: String,
    },
    RestartContainer {
        docker_proxy_url: String,
        container: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SecurityProfileConfig {
    #[serde(default)]
    pub allow_env_vars: bool,
    #[serde(default)]
    pub require_signature: bool,
    #[serde(default)]
    pub replay_tolerance_seconds: Option<i64>,
}
