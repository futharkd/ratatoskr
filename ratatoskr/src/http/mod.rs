use axum::response::IntoResponse;

pub mod webhook;

pub async fn healthz() -> impl IntoResponse {
    "ok"
}
