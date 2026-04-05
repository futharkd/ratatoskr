# Ratatoskr — development and testing

This document is for people working on the Ratatoskr crate. End-user documentation is in [README.md](README.md).

## Integration test layout

Cargo builds each `tests/*.rs` file as its own integration test binary:

| Binary | Purpose |
|--------|---------|
| `tests/healthz.rs` | `GET /healthz` smoke test |
| `tests/webhook.rs` | Webhook HTTP behavior (mock provider); modules under `tests/webhook/` |
| `tests/providers.rs` | Provider integration tests; modules under `tests/providers/` |

Shared helpers live in `tests/support/mod.rs` and are included from each root via `mod support;`. Subdirectory modules are wired with `#[path = "..."]` because Cargo only treats direct children of `tests/` as separate crates.

## Provider integration tests (Infisical)

The `providers` binary includes:

- **Hermetic (default):** A local HTTP stub (via [wiremock](https://docs.rs/wiremock)) implements Infisical-style login and raw-secrets responses. The real `InfisicalProvider` client is exercised end-to-end from `POST /webhooks/<provider>` through fetch and flat-file output. No cloud credentials are required.

- **Live (optional):** An ignored test calls the real Infisical API using environment variables.

### Running tests

```bash
cargo test -p ratatoskr
```

### Live Infisical (ignored test)

Set the variables below, then:

```bash
cargo test -p ratatoskr --test providers -- --ignored --nocapture
```

| Variable | Required | Description |
|----------|----------|-------------|
| `RATATOSKR_INFISICAL_API_BASE_URL` | yes | e.g. `https://app.infisical.com` or your self-hosted base URL |
| `RATATOSKR_INFISICAL_CLIENT_ID` | yes | Universal auth client ID |
| `RATATOSKR_INFISICAL_CLIENT_SECRET` | yes | Universal auth client secret |
| `RATATOSKR_INFISICAL_ENVIRONMENT` | yes | Slug of the environment to fetch (e.g. `dev`) |
| `RATATOSKR_INFISICAL_SECRET_PATH` | yes | Secret path in Infisical (e.g. `/my-app`) |
| `RATATOSKR_INFISICAL_EXPECT_KEY` | no | Secret key name that must appear in the response (default `AUTH_SECRET`) |
| `RATATOSKR_INFISICAL_WEBHOOK_SECRET` | no | Overrides the webhook signing secret in test config (not used by the live fetch test) |

Use environment variables or your shell’s secret mechanism only; do not commit credentials. A CI job can inject the same variables as protected secrets and run `cargo test -- --ignored` for continuous live checks if desired.
