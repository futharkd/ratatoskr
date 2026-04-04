use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::json;
use tracing::{error, info};

use crate::AppState;

pub async fn handle(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    match state
        .engine
        .process_webhook(&provider, &headers, &body)
        .await
    {
        Ok(result) => {
            info!(
                provider = provider,
                matched_services = result.matched_services,
                "webhook processed"
            );
            (StatusCode::OK, Json(json!(result))).into_response()
        }
        Err(err) => {
            error!(provider = provider, error = %err, "webhook processing failed");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}
