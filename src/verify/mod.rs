use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};
use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use http::HeaderMap;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn verify_infisical_signature(
    headers: &HeaderMap,
    raw_body: &Bytes,
    shared_secret: &str,
    tolerance_seconds: i64,
) -> anyhow::Result<()> {
    let signature_header = headers
        .get("x-infisical-signature")
        .ok_or_else(|| anyhow!("missing x-infisical-signature header"))?
        .to_str()
        .context("x-infisical-signature is not valid utf-8")?;

    let (timestamp, signature) = parse_signature_header(signature_header)?;
    validate_timestamp(timestamp, tolerance_seconds)?;
    verify_hmac(shared_secret, raw_body, timestamp, &signature)?;

    Ok(())
}

fn parse_signature_header(header: &str) -> anyhow::Result<(i64, String)> {
    let mut ts = None;
    let mut sig = None;
    for part in header.split(';') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or_default().trim();
        let value = kv.next().unwrap_or_default().trim();
        match key {
            "t" => {
                ts = Some(
                    value
                        .parse::<i64>()
                        .context("invalid signature timestamp")?,
                )
            }
            "signature" => sig = Some(value.to_string()),
            _ => {}
        }
    }

    Ok((
        ts.ok_or_else(|| anyhow!("signature header missing timestamp"))?,
        sig.ok_or_else(|| anyhow!("signature header missing signature"))?,
    ))
}

fn validate_timestamp(timestamp: i64, tolerance_seconds: i64) -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let delta = (now - timestamp).abs();
    if delta > tolerance_seconds {
        return Err(anyhow!(
            "stale signature timestamp: delta={}s, tolerance={}s",
            delta,
            tolerance_seconds
        ));
    }
    Ok(())
}

fn verify_hmac(
    secret: &str,
    raw_body: &Bytes,
    timestamp: i64,
    signature_hex: &str,
) -> anyhow::Result<()> {
    let payload = format!("{timestamp}.{}", String::from_utf8_lossy(raw_body));
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).context("invalid HMAC secret length")?;
    mac.update(payload.as_bytes());

    let expected = hex::decode(signature_hex).context("signature is not valid hex")?;
    mac.verify_slice(&expected)
        .map_err(|_| anyhow!("signature verification failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::HeaderValue;

    #[test]
    fn verifies_valid_header() {
        let secret = "top-secret";
        let body = Bytes::from(r#"{"event":"secrets.modified"}"#.as_bytes().to_vec());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let payload = format!("{timestamp}.{}", String::from_utf8_lossy(&body));
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-infisical-signature",
            HeaderValue::from_str(&format!("t={timestamp};signature={signature}")).unwrap(),
        );

        let result = verify_infisical_signature(&headers, &body, secret, 300);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_stale_timestamp() {
        let secret = "top-secret";
        let body = Bytes::from(r#"{"event":"secrets.modified"}"#.as_bytes().to_vec());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 1000;
        let payload = format!("{timestamp}.{}", String::from_utf8_lossy(&body));
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-infisical-signature",
            HeaderValue::from_str(&format!("t={timestamp};signature={signature}")).unwrap(),
        );

        let result = verify_infisical_signature(&headers, &body, secret, 300);
        assert!(result.is_err());
    }
}
