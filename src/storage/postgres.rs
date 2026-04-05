use std::time::SystemTime;

use anyhow::Context;
use async_trait::async_trait;
use tokio_postgres::{Client, NoTls};

use super::IdempotencyStore;

pub struct PostgresIdempotencyStore {
    client: Client,
}

impl PostgresIdempotencyStore {
    pub async fn new(postgres_url: &str) -> anyhow::Result<Self> {
        let (client, connection) = tokio_postgres::connect(postgres_url, NoTls)
            .await
            .context("failed connecting to postgres")?;

        tokio::spawn(async move {
            if let Err(err) = connection.await {
                tracing::error!(error = %err, "postgres connection error");
            }
        });

        let store = Self { client };
        store
            .init()
            .await
            .context("failed to initialize idempotency postgres store")?;
        Ok(store)
    }

    async fn init(&self) -> anyhow::Result<()> {
        self.client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS processed_events(
                    event_id TEXT PRIMARY KEY,
                    processed_at BIGINT NOT NULL
                );
                "#,
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl IdempotencyStore for PostgresIdempotencyStore {
    async fn mark_if_new(&self, event_id: &str) -> anyhow::Result<bool> {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs() as i64;

        let affected = self
            .client
            .execute(
                r#"
                INSERT INTO processed_events(event_id, processed_at)
                VALUES ($1, $2)
                ON CONFLICT(event_id) DO NOTHING
                "#,
                &[&event_id, &ts],
            )
            .await?;

        Ok(affected == 1)
    }
}
