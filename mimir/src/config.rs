use std::{fs, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::placeholders::PlaceholderPolicy;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MimirConfig {
    #[serde(default)]
    pub placeholders: PlaceholderOverride,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PlaceholderOverride {
    #[serde(default)]
    pub env: Option<bool>,
    #[serde(default)]
    pub file: Option<bool>,
}

impl MimirConfig {
    pub fn placeholder_policy(&self) -> PlaceholderPolicy {
        PlaceholderPolicy {
            allow_env_placeholders: self.placeholders.env.unwrap_or(false),
            allow_file_placeholders: self.placeholders.file.unwrap_or(false),
        }
    }

    pub fn with_fallbacks(mut self, defaults: &MimirConfig) -> Self {
        self.placeholders = self.placeholders.with_fallbacks(&defaults.placeholders);
        self
    }
}

impl PlaceholderOverride {
    pub fn with_fallbacks(mut self, defaults: &PlaceholderOverride) -> Self {
        if self.env.is_none() {
            self.env = defaults.env;
        }
        if self.file.is_none() {
            self.file = defaults.file;
        }
        self
    }
}

pub fn load_toml_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)
        .with_context(|| format!("unable to read config from {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("invalid TOML config at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{MimirConfig, load_toml_file};

    #[test]
    fn parses_mimir_config_defaults() {
        let cfg: MimirConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.placeholders.env, None);
        assert_eq!(cfg.placeholders.file, None);
    }

    #[test]
    fn parses_mimir_config_values() {
        let cfg: MimirConfig = toml::from_str(
            r#"
[placeholders]
env = true
file = false
"#,
        )
        .unwrap();
        assert_eq!(cfg.placeholders.env, Some(true));
        assert_eq!(cfg.placeholders.file, Some(false));
    }

    #[test]
    fn load_toml_file_reports_invalid_toml() {
        let err = load_toml_file::<MimirConfig>("/definitely/not/real.toml").unwrap_err();
        assert!(err.to_string().contains("unable to read config"));
    }

    #[test]
    fn applies_consumer_fallbacks() {
        let explicit = MimirConfig {
            placeholders: super::PlaceholderOverride {
                env: Some(true),
                file: None,
            },
        };
        let defaults = MimirConfig {
            placeholders: super::PlaceholderOverride {
                env: Some(false),
                file: Some(false),
            },
        };
        let merged = explicit.with_fallbacks(&defaults);
        assert_eq!(merged.placeholders.env, Some(true));
        assert_eq!(merged.placeholders.file, Some(false));
    }
}
