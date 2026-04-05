use crate::config::ServiceConfig;

use super::types::WebhookPayload;

pub(crate) fn match_services(
    services: &[ServiceConfig],
    provider_name: &str,
    payload: &WebhookPayload,
) -> Vec<ServiceConfig> {
    let mut out = Vec::new();
    for service in services {
        if service.provider != provider_name {
            continue;
        }
        if let Some(event_env) = &payload.environment
            && service.secret_selector.environment != *event_env
        {
            continue;
        }
        if let Some(event_path) = &payload.secret_path
            && service.secret_selector.secret_path != *event_path
        {
            tracing::warn!(
                service = service.name,
                expected_path = service.secret_selector.secret_path,
                event_path = event_path,
                "service skipped due to secret path mismatch"
            );
            continue;
        }
        out.push(service.clone());
    }
    out
}
