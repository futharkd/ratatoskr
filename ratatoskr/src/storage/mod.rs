use std::{path::PathBuf, time::SystemTime};

use anyhow::Context;
use rusqlite::{Connection, params};

#[derive(Clone)]
pub struct SqliteIdempotencyStore {
    path: PathBuf,
}

impl SqliteIdempotencyStore {
    pub async fn new(path: &str) -> anyhow::Result<Self> {
        let store = Self {
            path: PathBuf::from(path),
        };
        store
            .init()
            .await
            .context("failed to initialize idempotency sqlite store")?;
        Ok(store)
    }

    async fn init(&self) -> anyhow::Result<()> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = Connection::open(path)?;
            conn.execute_batch(
                r#"
                    CREATE TABLE IF NOT EXISTS processed_events(
                        event_id TEXT PRIMARY KEY,
                        processed_at INTEGER NOT NULL
                    );
                "#,
            )?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub async fn mark_if_new(&self, event_id: &str) -> anyhow::Result<bool> {
        let path = self.path.clone();
        let event_id = event_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = Connection::open(path)?;
            let ts = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs() as i64;
            let affected = conn.execute(
                "INSERT OR IGNORE INTO processed_events(event_id, processed_at) VALUES (?1, ?2)",
                params![event_id, ts],
            )?;
            Ok(affected == 1)
        })
        .await?
    }
}
