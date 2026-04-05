//! HTTP webhook integration tests (one `cargo test` section for all webhook cases).

#[path = "providers/infisical/fixture.rs"]
mod infisical_fixture;

mod support;

#[path = "webhook/bad_request.rs"]
mod bad_request;
#[path = "webhook/happy_path.rs"]
mod happy_path;
#[path = "webhook/idempotency.rs"]
mod idempotency;
