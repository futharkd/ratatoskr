use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::Context;
use serde::Deserialize;

use super::{
    app::AppConfig, includes::IncludeFiles, profile::SecurityProfileConfig,
    provider::ProviderConfig, service::ServiceConfig,
};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ProviderFragment {
    #[serde(default)]
    pub(crate) providers: Vec<ProviderConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ServiceFragment {
    #[serde(default)]
    pub(crate) services: Vec<ServiceConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ProfileFragment {
    #[serde(default)]
    pub(crate) security_profiles: HashMap<String, SecurityProfileConfig>,
}

pub(crate) fn merge_split_fragments(
    cfg: &mut AppConfig,
    includes: &IncludeFiles,
) -> anyhow::Result<()> {
    let mut provider_names: HashSet<String> =
        cfg.providers.iter().map(|p| p.name.clone()).collect();
    let mut service_names: HashSet<String> = cfg.services.iter().map(|s| s.name.clone()).collect();
    let mut profile_keys: HashSet<String> = cfg.security_profiles.keys().cloned().collect();

    for file in &includes.provider_files {
        let fragment: ProviderFragment = parse_fragment(file)?;
        for provider in fragment.providers {
            if !provider_names.insert(provider.name.clone()) {
                anyhow::bail!(
                    "duplicate provider name `{}` found while loading {}",
                    provider.name,
                    file.display()
                );
            }
            cfg.providers.push(provider);
        }
    }

    for file in &includes.service_files {
        let fragment: ServiceFragment = parse_fragment(file)?;
        for service in fragment.services {
            if !service_names.insert(service.name.clone()) {
                anyhow::bail!(
                    "duplicate service name `{}` found while loading {}",
                    service.name,
                    file.display()
                );
            }
            cfg.services.push(service);
        }
    }

    for file in &includes.profile_files {
        let fragment: ProfileFragment = parse_fragment(file)?;
        for (key, profile) in fragment.security_profiles {
            if !profile_keys.insert(key.clone()) {
                anyhow::bail!(
                    "duplicate security profile `{}` found while loading {}",
                    key,
                    file.display()
                );
            }
            cfg.security_profiles.insert(key, profile);
        }
    }

    Ok(())
}

pub(crate) fn parse_fragment<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("unable to read split config file {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid split config TOML at {}", path.display()))
}
