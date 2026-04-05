use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfisicalProviderConfig {
    #[serde(default = "default_infisical_base_url")]
    pub api_base_url: String,
    /// Infisical project / workspace id (UUID). Passed to the official SDK as `workspaceId`.
    pub project_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub webhook_secret: String,
}

fn default_infisical_base_url() -> String {
    "https://app.infisical.com".to_string()
}
