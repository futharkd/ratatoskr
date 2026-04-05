//! Infisical provider integration tests.
//!
//! - [`stub`] — Hermetic HTTP stubs (wiremock) + official SDK + full `POST /webhooks/...` dispatch.
//! - [`live`] — Opt-in tests against real Infisical (ignored by default).

mod live;
mod stub;
