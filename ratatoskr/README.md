# yggdrasil/ratatoskr

Ratatoskr is a configurable Rust webhook worker for secret delivery and lifecycle orchestration in lightweight Docker Compose deployments.

Ratatoskr owns its application schema, and uses the shared `mimir` crate for reusable config primitives (for example the `[mimir]` section and placeholder resolution/policy merging).

## What It Does

- Accepts signed webhooks from providers (Infisical first).
- Verifies HMAC-SHA256 signatures and replay window timestamps.
- Deduplicates webhook events with pluggable storage backends (SQLite default, PostgreSQL supported).
- Fetches scoped secrets through provider adapters.
- Renders outputs per service (flat files or templated YAML today).
- Applies lifecycle hooks (`no_action`, `reload_caddy`, `restart_container`).

## Quick Start

1. Set provider env vars referenced from the example config (see [`examples/config/providers/infisical_main.toml`](examples/config/providers/infisical_main.toml)):
   - `INFISICAL_PROJECT_ID` (project / workspace id from Infisical)
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
- `includes`: optional explicit glob lists for merging split provider/profile/service TOML fragments.
- `mimir`: shared library settings (for example placeholder defaults).
- `services`: service-by-service policy:
  - selector (`environment`, `secret_path`, optional key filters)
  - render mode and destination
  - lifecycle action
  - security profile binding
- `security_profiles`: optional named policy bundles for teams with mixed requirements.

## Modular Config (Split Files)

You can keep one main runtime config and merge additional TOML fragments using `[includes]`.

- **Only** the glob patterns you list are loaded (paths are relative to the main config file’s directory unless absolute).
- For each of `providers`, `security_profiles`, and `services`: omit the key or use an empty array to load **nothing** extra for that category (main file only).

Example:

```toml
[includes]
providers = ["config/providers/*.toml"]
security_profiles = ["config/profiles/*.toml"]
services = ["config/services/*.toml"]
```

**Breaking change:** earlier versions also merged implicit `config/providers/*.toml` (and sibling profile/service paths) even when not listed. You must list every glob you want merged.

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

- `[mimir.placeholders]` (global baseline for this worker config)
  - `env = false`
  - `file = false`
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