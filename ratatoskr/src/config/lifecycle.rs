use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LifecycleAction {
    #[default]
    NoAction,
    ReloadCaddy {
        admin_url: String,
    },
    RestartContainer {
        docker_proxy_url: String,
        container: String,
    },
}
