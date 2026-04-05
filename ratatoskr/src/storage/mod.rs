pub mod postgres;
pub mod sqlite;

use std::sync::Arc;

use async_trait::async_trait;

use mimir::config::{StorageBackend, StorageConfig};

#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    async fn mark_if_new(&self, event_id: &str) -> anyhow::Result<bool>;
}

pub async fn build_idempotency_store(
    config: &StorageConfig,
) -> anyhow::Result<Arc<dyn IdempotencyStore>> {
    match config.backend {
        StorageBackend::Sqlite => {
            let store = sqlite::SqliteIdempotencyStore::new(&config.sqlite_path).await?;
            Ok(Arc::new(store))
        }
        StorageBackend::Postgres => {
            let postgres_url = config.postgres_url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("storage.postgres_url is required for postgres backend")
            })?;
            let store = postgres::PostgresIdempotencyStore::new(postgres_url).await?;
            Ok(Arc::new(store))
        }
    }
}
