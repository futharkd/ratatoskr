use bytes::Bytes;
use http::HeaderMap;
use sha2::{Digest, Sha256};

pub(crate) fn build_event_key(provider_name: &str, headers: &HeaderMap, body: &Bytes) -> String {
    let delivery_id = headers
        .get("x-infisical-delivery-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(provider_name.as_bytes());
    hasher.update(delivery_id.as_bytes());
    hasher.update(body);
    hex::encode(hasher.finalize())
}
