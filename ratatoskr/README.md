# yggdrasil/ratatoskr

Ratatoskr is a configurable Rust webhook worker for secret delivery and lifecycle orchestration in lightweight Docker Compose deployments.

Configuration parsing and schema types are provided by the shared `mimir` crate in this repository.

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
- `includes`: optional split-file config globs for providers/profiles/services.
- `services`: service-by-service policy:
  - selector (`environment`, `secret_path`, optional key filters)
  - render mode and destination
  - lifecycle action
  - security profile binding
- `security_profiles`: optional named policy bundles for teams with mixed requirements.

## Modular Config (Split Files)

You can keep one main runtime config and load split files for providers, profiles, and services.

- Convention folders (auto-loaded relative to main config):
  - `config/providers/*.toml`
  - `config/profiles/*.toml`
  - `config/services/*.toml`
- Explicit include globs (optional) under `[includes]`:
  - `providers = ["..."]`
  - `security_profiles = ["..."]`
  - `services = ["..."]`

Duplicate names are rejected at startup:

- provider duplicate key: `provider.name`
- service duplicate key: `service.name`
- profile duplicate key: profile map key

See split examples in:

- `examples/ratatoskr.example.toml`
- `examples/config/providers/`
- `examples/config/profiles/`
- `examples/config/services/`

## Placeholder Injection

Ratatoskr supports Caddy-style placeholders in all render outputs (`flat_files` and `templated_yaml`):

- `{env:ENV_VAR}` to inject environment variable values
- `{file:/absolute/path/to/secret}` to inject file contents

Security defaults are deny-by-default:

- `[security_profiles.<name>.placeholders]`
  - `env = false`
  - `file = false`

Enable per profile in `security_profiles` and optionally override per service using:

- `placeholder_policy_override.env`
- `placeholder_policy_override.file`

### Security Profiles

- `strict`: file-based delivery, signature required, replay checks enabled.
- `env_only_allowed`: allows lower-sensitivity setups where env vars are accepted by policy.

You can mix profiles per service in one deployment without changing code.