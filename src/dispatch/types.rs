use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct DispatchResult {
    pub provider: String,
    pub matched_services: usize,
    pub applied_services: usize,
    pub skipped_duplicate: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WebhookPayload {
    #[allow(dead_code)]
    pub(crate) event: Option<String>,
    #[serde(default)]
    pub(crate) environment: Option<String>,
    #[serde(rename = "secretPath", default)]
    pub(crate) secret_path: Option<String>,
}
