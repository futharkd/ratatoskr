use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;

use super::app::AppConfig;
use super::fragments::merge_split_fragments;
use super::includes::resolve_includes;

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(&path)
            .with_context(|| format!("unable to read config from {}", path.display()))?;
        let mut cfg: AppConfig = toml::from_str(&content)
            .with_context(|| format!("invalid TOML config at {}", path.display()))?;
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        validate_unique_inline_names(&cfg)?;
        let includes = resolve_includes(&base_dir, &cfg.includes)?;
        merge_split_fragments(&mut cfg, &includes)?;
        cfg.apply_defaults();
        Ok(cfg)
    }
}

fn validate_unique_inline_names(cfg: &AppConfig) -> anyhow::Result<()> {
    let mut provider_names = HashSet::new();
    for provider in &cfg.providers {
        if !provider_names.insert(provider.name.clone()) {
            anyhow::bail!("duplicate provider name in main config: {}", provider.name);
        }
    }

    let mut service_names = HashSet::new();
    for service in &cfg.services {
        if !service_names.insert(service.name.clone()) {
            anyhow::bail!("duplicate service name in main config: {}", service.name);
        }
    }
    Ok(())
}
