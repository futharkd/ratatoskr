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

Layout under `tests/providers/infisical/`:

| File | Module prefix | Role |
|------|---------------|------|
| `mod.rs` | `infisical::` | Wires `stub` and `live` (included from `tests/providers.rs` via `#[path = "providers/infisical/mod.rs"]`). |
| `stub.rs` | `infisical::stub::` | Hermetic E2E: wiremock + real Infisical SDK + full webhook dispatch to disk. |
| `live.rs` | `infisical::live::` | Ignored test: **outbound-only** real `fetch_secrets` against Infisical (env / `.env`). Does not start a listener or receive webhooks. |

**Testing policy:** **Live** tests only exercise **outbound** provider calls (no exposing Ratatoskr on a public URL, no tunnels). **Webhook** coverage is **hermetic**: [`tests/webhook/`](tests/webhook/) (mock provider: signatures, idempotency, routing) and `infisical::stub::*` (real Infisical SDK + wiremock + in-process `POST /webhooks/<provider>` through fetch and disk). That stays true in CI and in this document—we do not document ngrok, Cloudflare Quick Tunnel, or similar as part of the official workflow.

**How this relates to `tests/webhook/`:** The `webhook` binary uses a mock provider to exercise HTTP concerns—signature verification, idempotency, and a happy path—without talking to Infisical. `infisical::stub::*` exercises the **real** SDK and dispatch stack against a fake HTTP server, so it catches provider-specific request/response shape and wiring bugs that mocks would not.

The `providers` binary includes:

- **Hermetic (default):** A local HTTP stub (via [wiremock](https://docs.rs/wiremock)) matches the HTTP used by the official [`infisical`](https://crates.io/crates/infisical) Rust SDK (universal-auth login + list secrets). The real `InfisicalProvider` is exercised end-to-end from `POST /webhooks/<provider>` through fetch and flat-file output. No cloud credentials are required.

- **Live (optional):** An ignored test calls the real Infisical API **from the test process** (SDK `fetch_secrets` only), using environment variables and/or a local `.env` file.

### Running tests

```bash
cargo test -p ratatoskr
```

### Live Infisical (ignored test, outbound only)

The live test calls Infisical over HTTPS from the test binary; it does **not** open `listen_addr` or validate that Infisical can reach you. Webhook delivery from Infisical Cloud to a laptop would require a public URL or self-hosted topology and is **out of scope** for this test and for the workflows described here.

Set the variables below (in your shell or in a **`.env` file**), then:

```bash
cargo test -p ratatoskr --test providers -- --ignored --nocapture
```

#### `.env` file

Create `ratatoskr/.env` at the crate root (gitignored) with the same keys as the table, one per line:

```env
RATATOSKR_INFISICAL_API_BASE_URL=https://app.infisical.com
RATATOSKR_INFISICAL_PROJECT_ID=...
RATATOSKR_INFISICAL_CLIENT_ID=...
RATATOSKR_INFISICAL_CLIENT_SECRET=...
RATATOSKR_INFISICAL_ENVIRONMENT=dev
RATATOSKR_INFISICAL_SECRET_PATH=/my-app
# optional:
# RATATOSKR_INFISICAL_EXPECT_KEY=MY_KEY
# RATATOSKR_INFISICAL_WEBHOOK_SECRET=...
```

The live test loads that file from the crate root (`CARGO_MANIFEST_DIR`) before reading the environment. Variables that are **already set** in the process environment are **not** overwritten by the file.

| Variable | Required | Description |
|----------|----------|-------------|
| `RATATOSKR_INFISICAL_API_BASE_URL` | yes | e.g. `https://app.infisical.com` or your self-hosted base URL |
| `RATATOSKR_INFISICAL_PROJECT_ID` | yes | Infisical project / workspace id (passed to the SDK as `workspaceId`) |
| `RATATOSKR_INFISICAL_CLIENT_ID` | yes | Universal auth client ID |
| `RATATOSKR_INFISICAL_CLIENT_SECRET` | yes | Universal auth client secret |
| `RATATOSKR_INFISICAL_ENVIRONMENT` | yes | Slug of the environment to fetch (e.g. `dev`) |
| `RATATOSKR_INFISICAL_SECRET_PATH` | yes | Secret path in Infisical (e.g. `/my-app`) |
| `RATATOSKR_INFISICAL_EXPECT_KEY` | no | Secret key name that must appear in the response (default `AUTH_SECRET`) |
| `RATATOSKR_INFISICAL_WEBHOOK_SECRET` | no | Overrides the webhook signing secret in test config (not used by the live fetch test) |

Do not commit `.env` or real credentials; use your shell, `.env`, or a secret manager locally.

Runtime Infisical access uses the official Rust SDK ([crate](https://crates.io/crates/infisical), [docs](https://infisical.com/docs/sdks/languages/rust)). Provider config requires `project_id` (workspace id); the old `login_path` / `secrets_path` fields were removed because the SDK selects API routes.

## GitHub Actions

The repository workflow [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs on pushes and pull requests to `main`, and can be triggered manually with **workflow_dispatch**.

### Default job (`lint-and-test`)

Runs for every trigger: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (mimir + ratatoskr, including hermetic provider tests).

### Live Testing job (`live-tests`)

Runs after `lint-and-test` succeeds. It executes:

`cargo test -p ratatoskr --test providers -- --ignored --nocapture`

**Fork pull requests:** the `live-tests` job is skipped when the PR head branch lives in a fork (`github.event.pull_request.head.repo.full_name != github.repository`), so the workflow does not assume access to this repository’s Actions secrets or variables in that context. The main lint/test job still runs.

**Same-repository branches:** the live job runs. Configure **Actions** settings (Settings → Secrets and variables → Actions):

- **Repository variables** (non-sensitive config — visible to anyone who can read workflow runs/logs; do not put credentials here):

| Variable | Required for live job |
|----------|------------------------|
| `RATATOSKR_INFISICAL_API_BASE_URL` | yes |
| `RATATOSKR_INFISICAL_PROJECT_ID` | yes |
| `RATATOSKR_INFISICAL_ENVIRONMENT` | yes |
| `RATATOSKR_INFISICAL_SECRET_PATH` | yes |

- **Repository secrets** (universal auth credentials only):

| Secret | Required for live job |
|--------|------------------------|
| `RATATOSKR_INFISICAL_CLIENT_ID` | yes |
| `RATATOSKR_INFISICAL_CLIENT_SECRET` | yes |

The workflow maps these into the same environment variable names the live test reads (`vars.*` / `secrets.*` in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)).

Optional test inputs (`RATATOSKR_INFISICAL_EXPECT_KEY`, `RATATOSKR_INFISICAL_WEBHOOK_SECRET`) are not set in CI by default; add them to the workflow `env` block from `vars.*` if you need them, or rely on the test’s defaults.

If required values are missing on a same-repo run, the ignored test fails when reading environment variables—add the variables and secrets above to make the job pass.
