# yggdrasil/ratatoskr

Ratatoskr is a configurable Rust webhook worker for secret delivery and lifecycle orchestration in lightweight Docker Compose deployments.

## What It Does

- Accepts signed webhooks from providers (Infisical first).
- Verifies HMAC-SHA256 signatures and replay window timestamps.
- Deduplicates webhook events with pluggable storage backends (SQLite default, PostgreSQL supported).
- Fetches scoped secrets through provider adapters.
- Renders outputs per service (flat files or templated YAML today).
- Applies lifecycle hooks (`no_action`, `reload_caddy`, `restart_container`).

## Quick Start

1. Set provider credentials and webhook secret env vars:
   - `INFISICAL_CLIENT_ID`
   - `INFISICAL_CLIENT_SECRET`
   - `INFISICAL_WEBHOOK_SECRET`
2. Copy and adapt [`examples/ratatoskr.example.toml`](examples/ratatoskr.example.toml).
3. Run:
   - `cargo run -- examples/ratatoskr.example.toml`
4. Send signed webhook payloads to:
   - `POST /webhooks/<provider-name>`

## Configuration Model

Ratatoskr is data-driven:

- `providers`: authentication and fetch backends.
- `defaults`: safe baseline behavior (replay window, retries, timeout).
- `storage`: idempotency backend (`sqlite` or `postgres`).
- `services`: service-by-service policy:
  - selector (`environment`, `secret_path`, optional key filters)
  - render mode and destination
  - lifecycle action
  - security profile binding
- `security_profiles`: optional named policy bundles for teams with mixed requirements.

### Security Profiles

- `strict`: file-based delivery, signature required, replay checks enabled.
- `env_only_allowed`: allows lower-sensitivity setups where env vars are accepted by policy.

You can mix profiles per service in one deployment without changing code.