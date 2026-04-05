//! Application configuration schema and split-file loading.

pub use mimir::config::{MimirConfig, PlaceholderOverride};

mod app;
mod defaults;
mod fragments;
mod includes;
mod lifecycle;
mod load;
mod output;
mod profile;
mod provider;
mod server;
mod service;
mod storage;

pub use app::AppConfig;
// Used by `#[cfg(test)]` modules and external callers; plain `cargo check` skips those paths.
#[allow(unused_imports)]
pub use defaults::DefaultsConfig;
#[allow(unused_imports)]
pub use includes::ConfigIncludes;
pub use lifecycle::LifecycleAction;
pub use output::OutputConfig;
#[allow(unused_imports)]
pub use profile::SecurityProfileConfig;
pub use provider::{ProviderConfig, ProviderKind};
#[allow(unused_imports)]
pub use server::ServerConfig;
pub use service::{SecretSelector, ServiceConfig};
pub use storage::{StorageBackend, StorageConfig};

#[allow(unused_imports)]
pub use crate::providers::infisical::InfisicalProviderConfig;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::AppConfig;

    #[test]
    fn loads_repository_example_config() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let example = root.join("examples/ratatoskr.example.toml");
        let cfg = AppConfig::load(&example).unwrap();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.services.len(), 3);
        assert!(cfg.security_profiles.contains_key("strict"));
        assert!(cfg.security_profiles.contains_key("env_only_allowed"));
    }

    #[test]
    fn loads_mimir_section_defaults() {
        let cfg: AppConfig = toml::from_str(
            r#"
[server]
listen_addr = "127.0.0.1:8080"

[storage]
backend = "sqlite"
sqlite_path = "./ratatoskr.db"
"#,
        )
        .unwrap();
        assert_eq!(cfg.mimir.placeholders.env, None);
        assert_eq!(cfg.mimir.placeholders.file, None);
    }
}
