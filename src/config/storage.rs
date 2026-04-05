use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub backend: StorageBackend,
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
    #[serde(default)]
    pub postgres_url: Option<String>,
}

fn default_sqlite_path() -> String {
    "./ratatoskr.db".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    #[default]
    Sqlite,
    Postgres,
}
