use anyhow::anyhow;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct LoginResponse {
    #[serde(rename = "accessToken")]
    pub(super) access_token: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SecretItem {
    #[serde(rename = "secretKey")]
    pub(super) secret_key: String,
    #[serde(rename = "secretValue")]
    pub(super) secret_value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SecretEnvelope {
    #[serde(default)]
    secrets: Vec<SecretItem>,
}

pub(super) fn parse_secret_items(payload: Value) -> anyhow::Result<Vec<SecretItem>> {
    if let Ok(parsed) = serde_json::from_value::<SecretEnvelope>(payload.clone()) {
        return Ok(parsed.secrets);
    }

    let secrets = payload
        .get("secrets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("missing `secrets` array in provider response"))?;
    let mut items = Vec::new();
    for secret in secrets {
        let key = secret
            .get("secretKey")
            .or_else(|| secret.get("key"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("secret entry missing key"))?
            .to_string();
        let value = secret
            .get("secretValue")
            .or_else(|| secret.get("value"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        items.push(SecretItem {
            secret_key: key,
            secret_value: value,
        });
    }
    Ok(items)
}
