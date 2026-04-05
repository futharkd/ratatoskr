# Ratatoskr — development and testing

This document is for people working on the Ratatoskr crate. End-user documentation is in [README.md](README.md).

## Integration test layout

Cargo builds each `tests/*.rs` file as its own integration test binary:

| Binary | Purpose |
|--------|---------|
| `tests/healthz.rs` | `GET /healthz` smoke test |
| `tests/webhook.rs` | Webhook HTTP behavior (mock provider); modules under `tests/webhook/` |
| `tests/providers.rs` | Provider integration tests; modules under `tests/providers/` |

**`tests/support/mod.rs`** holds integration helpers that are **not** tied to a specific provider: empty/minimal `AppConfig`, `DispatchEngine` builders, [`MockProvider`](tests/support/mod.rs), and the Axum router wrapper. Each test root includes it with `mod support;`.

Provider-specific sample config and Infisical-style webhook signing live under **`tests/providers/infisical/fixture.rs`**. That file is reused from the `webhook` integration crate via `#[path = "providers/infisical/fixture.rs"]` so Infisical-shaped tests do not bloat generic support.

Subdirectory modules are wired with `#[path = "..."]` where needed because Cargo only treats direct children of `tests/` as separate crates.

## Provider integration tests (Infisical)

Layout under `tests/providers/infisical/`:

| File | Module prefix | Role |
|------|---------------|------|
| `mod.rs` | `infisical::` | Wires `fixture` + `stub` (included from `tests/providers.rs` via `#[path = "providers/infisical/mod.rs"]`). |
| `fixture.rs` | `infisical::fixture` (or `crate::infisical_fixture` from `tests/webhook.rs`) | Infisical-only: sample `AppConfig`, signing headers, shared constants. |
| `stub.rs` | `infisical::stub::` | Hermetic E2E: wiremock + real Infisical SDK + full webhook dispatch to disk. |

**Policy:** All automated Infisical coverage is **hermetic** (wiremock + official SDK). No ignored “live cloud” tests and no CI secrets for Infisical accounts.

**How this relates to `tests/webhook/`:** The `webhook` binary uses [`MockProvider`](tests/support/mod.rs) and [`fixture.rs`](tests/providers/infisical/fixture.rs) to exercise signature verification, idempotency, and routing without the Infisical SDK. `infisical::stub::*` runs the **real** SDK and dispatch stack against a fake HTTP server.

### Running tests

```bash
cargo test -p ratatoskr
```

Runtime shape for real deployments uses the official Rust SDK ([crate](https://crates.io/crates/infisical), [docs](https://infisical.com/docs/sdks/languages/rust)). Provider config requires `project_id` (workspace id); the old `login_path` / `secrets_path` fields were removed because the SDK selects API routes.

## GitHub Actions

The repository workflow [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs on pushes and pull requests to `main`, and can be triggered manually with **workflow_dispatch**.

The **lint-and-test** job runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (mimir + ratatoskr, including hermetic provider tests).
