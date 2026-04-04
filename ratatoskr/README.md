# yggdrasil/ratatoskr

Ratatoskr receives signed webhooks from cloud services, verifies payloads with HMAC-SHA256, and safely executes predefined sync jobs (git deploys to pinned SHAs, SOPS/age secret materialization) with systemd sandboxing, SQLite/PostgreSQL idempotency tracking, and atomic filesystem operations.

Allows setting custom logic (though defaults are given) for specific operations, from different cloud providers (e.g. GitHub for Git repos, Infisical for secrets management, ...)