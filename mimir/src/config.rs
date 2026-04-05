use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use glob::glob;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub includes: ConfigIncludes,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    #[serde(default)]
    pub security_profiles: HashMap<String, SecurityProfileConfig>,
}

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
        let includes = discover_includes(&base_dir, &cfg.includes)?;
        merge_split_fragments(&mut cfg, &includes)?;
        cfg.apply_defaults();
        Ok(cfg)
    }

    fn apply_defaults(&mut self) {
        if self.defaults.replay_tolerance_seconds == 0 {
            self.defaults.replay_tolerance_seconds = 300;
        }
        if self.defaults.http_timeout_seconds == 0 {
            self.defaults.http_timeout_seconds = 10;
        }
        if self.defaults.max_retries == 0 {
            self.defaults.max_retries = 3;
        }
        if self.defaults.retry_backoff_millis == 0 {
            self.defaults.retry_backoff_millis = 300;
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConfigIncludes {
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub security_profiles: Vec<String>,
}

#[derive(Debug, Clone)]
struct IncludeFiles {
    provider_files: Vec<PathBuf>,
    service_files: Vec<PathBuf>,
    profile_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub replay_tolerance_seconds: i64,
    #[serde(default)]
    pub http_timeout_seconds: u64,
    #[serde(default)]
    pub max_retries: usize,
    #[serde(default)]
    pub retry_backoff_millis: u64,
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(flatten)]
    pub kind: ProviderKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderKind {
    Infisical(InfisicalProviderConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InfisicalProviderConfig {
    #[serde(default = "default_infisical_base_url")]
    pub api_base_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub webhook_secret: String,
    #[serde(default = "default_login_path")]
    pub login_path: String,
    #[serde(default = "default_secrets_path")]
    pub secrets_path: String,
}

fn default_infisical_base_url() -> String {
    "https://app.infisical.com".to_string()
}

fn default_login_path() -> String {
    "/api/v1/auth/universal-auth/login".to_string()
}

fn default_secrets_path() -> String {
    "/api/v3/secrets/raw".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub name: String,
    pub provider: String,
    pub secret_selector: SecretSelector,
    pub output: OutputConfig,
    #[serde(default)]
    pub lifecycle: LifecycleAction,
    #[serde(default)]
    pub security_profile: String,
    #[serde(default)]
    pub placeholder_policy_override: Option<PlaceholderPolicyOverride>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecretSelector {
    pub environment: String,
    #[serde(default = "default_secret_path")]
    pub secret_path: String,
    #[serde(default)]
    pub include_keys: Vec<String>,
}

fn default_secret_path() -> String {
    "/".to_string()
}

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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SecurityProfileConfig {
    #[serde(default)]
    pub allow_env_vars: bool,
    #[serde(default)]
    pub require_signature: bool,
    #[serde(default)]
    pub replay_tolerance_seconds: Option<i64>,
    #[serde(default)]
    pub placeholders: ProfilePlaceholderPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PlaceholderPolicyOverride {
    #[serde(default)]
    pub env: Option<bool>,
    #[serde(default)]
    pub file: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProfilePlaceholderPolicy {
    #[serde(default)]
    pub env: bool,
    #[serde(default)]
    pub file: bool,
}

#[derive(Debug, Default, Deserialize)]
struct ProviderFragment {
    #[serde(default)]
    providers: Vec<ProviderConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct ServiceFragment {
    #[serde(default)]
    services: Vec<ServiceConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct ProfileFragment {
    #[serde(default)]
    security_profiles: HashMap<String, SecurityProfileConfig>,
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

fn discover_includes(base_dir: &Path, includes: &ConfigIncludes) -> anyhow::Result<IncludeFiles> {
    let provider_files = collect_files(base_dir, "config/providers/*.toml", &includes.providers)?;
    let service_files = collect_files(base_dir, "config/services/*.toml", &includes.services)?;
    let profile_files = collect_files(
        base_dir,
        "config/profiles/*.toml",
        &includes.security_profiles,
    )?;
    Ok(IncludeFiles {
        provider_files,
        service_files,
        profile_files,
    })
}

fn collect_files(
    base_dir: &Path,
    convention: &str,
    explicit_globs: &[String],
) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    let convention_pattern = base_dir.join(convention).to_string_lossy().to_string();
    files.extend(expand_glob(&convention_pattern)?);

    for pattern in explicit_globs {
        let full_pattern = if Path::new(pattern).is_absolute() {
            pattern.clone()
        } else {
            base_dir.join(pattern).to_string_lossy().to_string()
        };
        files.extend(expand_glob(&full_pattern)?);
    }

    Ok(files.into_iter().collect())
}

fn expand_glob(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    for entry in glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))? {
        let path =
            entry.with_context(|| format!("glob expansion failed for pattern: {pattern}"))?;
        if path.is_file() {
            results.push(path);
        }
    }
    Ok(results)
}

fn merge_split_fragments(cfg: &mut AppConfig, includes: &IncludeFiles) -> anyhow::Result<()> {
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

fn parse_fragment<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("unable to read split config file {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid split config TOML at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::AppConfig;

    #[test]
    fn loads_single_file_config_without_includes() {
        let temp = tempdir().unwrap();
        let main_cfg = temp.path().join("main.toml");
        fs::write(
            &main_cfg,
            r#"
[server]
listen_addr = "127.0.0.1:8080"

[defaults]
replay_tolerance_seconds = 300
http_timeout_seconds = 10
max_retries = 3
retry_backoff_millis = 300

[storage]
backend = "sqlite"
sqlite_path = "./ratatoskr.db"

[[providers]]
name = "infisical_main"
type = "infisical"
api_base_url = "https://app.infisical.com"
client_id = "{env:INFISICAL_CLIENT_ID}"
client_secret = "{env:INFISICAL_CLIENT_SECRET}"
webhook_secret = "{env:INFISICAL_WEBHOOK_SECRET}"
login_path = "/api/v1/auth/universal-auth/login"
secrets_path = "/api/v3/secrets/raw"

[security_profiles.strict]
allow_env_vars = false
require_signature = true

[[services]]
name = "caddy"
provider = "infisical_main"
security_profile = "strict"
lifecycle = { action = "no_action" }
secret_selector = { environment = "prod", secret_path = "/caddy", include_keys = ["TOKEN"] }
output = { mode = "flat_files", directory = "/tmp/secrets", file_mode = 256 }
"#,
        )
        .unwrap();

        let cfg = AppConfig::load(&main_cfg).unwrap();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.services.len(), 1);
        assert!(cfg.security_profiles.contains_key("strict"));
    }

    #[test]
    fn loads_hybrid_split_config() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config/providers")).unwrap();
        fs::create_dir_all(temp.path().join("config/services")).unwrap();
        fs::create_dir_all(temp.path().join("config/profiles")).unwrap();
        fs::create_dir_all(temp.path().join("extra/services")).unwrap();

        fs::write(
            temp.path().join("config/providers/infisical.toml"),
            r#"
[[providers]]
name = "infisical_main"
type = "infisical"
api_base_url = "https://app.infisical.com"
client_id = "{env:INFISICAL_CLIENT_ID}"
client_secret = "{env:INFISICAL_CLIENT_SECRET}"
webhook_secret = "{env:INFISICAL_WEBHOOK_SECRET}"
login_path = "/api/v1/auth/universal-auth/login"
secrets_path = "/api/v3/secrets/raw"
"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("config/profiles/strict.toml"),
            r#"
[security_profiles.strict]
allow_env_vars = false
require_signature = true
"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("config/services/caddy.toml"),
            r#"
[[services]]
name = "caddy"
provider = "infisical_main"
security_profile = "strict"
lifecycle = { action = "no_action" }
secret_selector = { environment = "prod", secret_path = "/caddy", include_keys = ["TOKEN"] }
output = { mode = "flat_files", directory = "/tmp/secrets", file_mode = 256 }
"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("extra/services/papra.toml"),
            r#"
[[services]]
name = "papra"
provider = "infisical_main"
security_profile = "strict"
lifecycle = { action = "no_action" }
secret_selector = { environment = "prod", secret_path = "/papra", include_keys = ["AUTH_SECRET"] }
output = { mode = "flat_files", directory = "/tmp/secrets", file_mode = 256 }
"#,
        )
        .unwrap();
        let main_cfg = temp.path().join("main.toml");
        fs::write(
            &main_cfg,
            r#"
[server]
listen_addr = "127.0.0.1:8080"

[defaults]
replay_tolerance_seconds = 300
http_timeout_seconds = 10
max_retries = 3
retry_backoff_millis = 300

[storage]
backend = "sqlite"
sqlite_path = "./ratatoskr.db"

[includes]
services = ["extra/services/*.toml"]
"#,
        )
        .unwrap();

        let cfg = AppConfig::load(&main_cfg).unwrap();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.services.len(), 2);
        assert!(cfg.security_profiles.contains_key("strict"));
    }

    #[test]
    fn fails_on_duplicate_service_name() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("config/services")).unwrap();
        let main_cfg = temp.path().join("main.toml");

        fs::write(
            temp.path().join("config/services/one.toml"),
            r#"
[[services]]
name = "duplicate"
provider = "infisical_main"
security_profile = "strict"
lifecycle = { action = "no_action" }
secret_selector = { environment = "prod", secret_path = "/a", include_keys = [] }
output = { mode = "flat_files", directory = "/tmp/a", file_mode = 256 }
"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("config/services/two.toml"),
            r#"
[[services]]
name = "duplicate"
provider = "infisical_main"
security_profile = "strict"
lifecycle = { action = "no_action" }
secret_selector = { environment = "prod", secret_path = "/b", include_keys = [] }
output = { mode = "flat_files", directory = "/tmp/b", file_mode = 256 }
"#,
        )
        .unwrap();
        fs::write(
            &main_cfg,
            r#"
[server]
listen_addr = "127.0.0.1:8080"

[defaults]
replay_tolerance_seconds = 300
http_timeout_seconds = 10
max_retries = 3
retry_backoff_millis = 300

[storage]
backend = "sqlite"
sqlite_path = "./ratatoskr.db"
"#,
        )
        .unwrap();

        let err = AppConfig::load(&main_cfg).unwrap_err();
        assert!(err.to_string().contains("duplicate service name"));
    }

    #[test]
    fn loads_repository_example_config() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("mimir crate should be inside workspace root")
            .to_path_buf();
        let example = root.join("ratatoskr/examples/ratatoskr.example.toml");
        let cfg = AppConfig::load(&example).unwrap();
        assert_eq!(cfg.providers.len(), 1);
        assert_eq!(cfg.services.len(), 3);
        assert!(cfg.security_profiles.contains_key("strict"));
        assert!(cfg.security_profiles.contains_key("env_only_allowed"));
    }
}
