use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OutputConfig {
    FlatFiles {
        directory: String,
        #[serde(default)]
        file_mode: Option<u32>,
    },
    TemplatedYaml {
        file_path: String,
        template: String,
        #[serde(default)]
        file_mode: Option<u32>,
    },
}
