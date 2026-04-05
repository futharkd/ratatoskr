use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfisicalProviderConfig {
    #[serde(default = "default_infisical_base_url")]
    pub api_base_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub webhook_secret: String,
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
