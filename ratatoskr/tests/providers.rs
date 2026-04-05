//! Provider integration tests (Infisical stub and optional live API). See `DEVELOPMENT.md` in the crate root.

mod support;

#[path = "providers/infisical_live.rs"]
mod infisical_live;
#[path = "providers/infisical_stub_e2e.rs"]
mod infisical_stub_e2e;
