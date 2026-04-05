use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use http::{HeaderMap, header::HeaderValue};
use sha2::Sha256;
use tempfile::tempdir;

use crate::{
    config::{
        AppConfig, ConfigIncludes, DefaultsConfig, InfisicalProviderConfig, LifecycleAction,
        MimirConfig, OutputConfig, PlaceholderOverride, ProviderConfig, ProviderKind,
        SecretSelector, SecurityProfileConfig, ServerConfig, ServiceConfig, StorageBackend,
        StorageConfig,
    },
    providers::{ProviderClient, SecretFetchRequest, SecretMap},
    storage::{IdempotencyStore, sqlite::SqliteIdempotencyStore},
};

use super::DispatchEngine;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct MockProvider {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ProviderClient for MockProvider {
    async fn fetch_secrets(&self, _request: SecretFetchRequest) -> anyhow::Result<SecretMap> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(BTreeMap::from([(
            "AUTH_SECRET".to_string(),
            "value-1".to_string(),
        )]))
    }

    fn provider_name(&self) -> &str {
        "infisical_main"
    }

    fn webhook_secret(&self) -> anyhow::Result<String> {
        Ok("top-secret".to_string())
    }
}

#[tokio::test]
async fn deduplicates_duplicate_webhook_events() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("idempotency.db");
    let output_dir = temp.path().join("secrets");

    let cfg = AppConfig {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        defaults: DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        storage: StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: db_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![ProviderConfig {
            name: "infisical_main".to_string(),
            kind: ProviderKind::Infisical(InfisicalProviderConfig {
                api_base_url: "https://app.infisical.com".to_string(),
                client_id: "x".to_string(),
                client_secret: "y".to_string(),
                webhook_secret: "top-secret".to_string(),
                login_path: "/api/v1/auth/universal-auth/login".to_string(),
                secrets_path: "/api/v3/secrets/raw".to_string(),
            }),
        }],
        services: vec![ServiceConfig {
            name: "papra".to_string(),
            provider: "infisical_main".to_string(),
            secret_selector: SecretSelector {
                environment: "prod".to_string(),
                secret_path: "/papra".to_string(),
                include_keys: Vec::new(),
            },
            output: OutputConfig::FlatFiles {
                directory: output_dir.to_string_lossy().into_owned(),
                file_mode: None,
            },
            lifecycle: LifecycleAction::NoAction,
            security_profile: "strict".to_string(),
            placeholder_policy_override: None,
        }],
        security_profiles: Default::default(),
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(MockProvider {
        calls: calls.clone(),
    });
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    providers.insert("infisical_main".to_string(), provider);
    let store: Arc<dyn IdempotencyStore> = Arc::new(
        SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap(),
    );
    let engine = DispatchEngine::new(cfg, providers, store);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);
    let headers = signed_headers("top-secret", &body);

    let first = engine
        .process_webhook("infisical_main", &headers, &body)
        .await
        .unwrap();
    let second = engine
        .process_webhook("infisical_main", &headers, &body)
        .await
        .unwrap();

    assert_eq!(first.applied_services, 1);
    assert!(second.skipped_duplicate);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn signed_headers(secret: &str, body: &Bytes) -> HeaderMap {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let payload = format!("{timestamp}.{}", String::from_utf8_lossy(body));
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-infisical-signature",
        HeaderValue::from_str(&format!("t={timestamp};signature={signature}")).unwrap(),
    );
    headers.insert(
        "x-infisical-delivery-id",
        HeaderValue::from_static("delivery-1"),
    );
    headers
}

#[tokio::test]
async fn applies_profile_placeholder_policy() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("idempotency-profile.db");
    let out_file = temp.path().join("output.yaml");
    let mut security_profiles = std::collections::HashMap::new();
    security_profiles.insert(
        "profile_with_env".to_string(),
        SecurityProfileConfig {
            allow_env_vars: false,
            require_signature: true,
            replay_tolerance_seconds: Some(300),
            placeholders: PlaceholderOverride {
                env: Some(true),
                file: Some(false),
            },
        },
    );

    let cfg = AppConfig {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        defaults: DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        storage: StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: db_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![ProviderConfig {
            name: "infisical_main".to_string(),
            kind: ProviderKind::Infisical(InfisicalProviderConfig {
                api_base_url: "https://app.infisical.com".to_string(),
                client_id: "x".to_string(),
                client_secret: "y".to_string(),
                webhook_secret: "top-secret".to_string(),
                login_path: "/api/v1/auth/universal-auth/login".to_string(),
                secrets_path: "/api/v3/secrets/raw".to_string(),
            }),
        }],
        services: vec![ServiceConfig {
            name: "profile-policy".to_string(),
            provider: "infisical_main".to_string(),
            secret_selector: SecretSelector {
                environment: "prod".to_string(),
                secret_path: "/papra".to_string(),
                include_keys: Vec::new(),
            },
            output: OutputConfig::TemplatedYaml {
                file_path: out_file.to_string_lossy().into_owned(),
                template: "value: {env:RATATOSKR_POLICY_ENV}\n".to_string(),
                file_mode: None,
            },
            lifecycle: LifecycleAction::NoAction,
            security_profile: "profile_with_env".to_string(),
            placeholder_policy_override: None,
        }],
        security_profiles,
    };

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("RATATOSKR_POLICY_ENV", "from-profile") };
    let provider = Arc::new(MockProvider {
        calls: Arc::new(AtomicUsize::new(0)),
    });
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    providers.insert("infisical_main".to_string(), provider);
    let store: Arc<dyn IdempotencyStore> = Arc::new(
        SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap(),
    );
    let engine = DispatchEngine::new(cfg, providers, store);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);
    let headers = signed_headers("top-secret", &body);
    engine
        .process_webhook("infisical_main", &headers, &body)
        .await
        .unwrap();

    let rendered = std::fs::read_to_string(out_file).unwrap();
    assert!(rendered.contains("from-profile"));
}

#[tokio::test]
async fn service_override_takes_precedence_over_profile() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("idempotency-override.db");
    let out_file = temp.path().join("output.yaml");
    let mut security_profiles = std::collections::HashMap::new();
    security_profiles.insert(
        "strict".to_string(),
        SecurityProfileConfig {
            allow_env_vars: false,
            require_signature: true,
            replay_tolerance_seconds: Some(300),
            placeholders: PlaceholderOverride {
                env: Some(false),
                file: Some(false),
            },
        },
    );

    let cfg = AppConfig {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(),
        },
        defaults: DefaultsConfig {
            replay_tolerance_seconds: 300,
            http_timeout_seconds: 10,
            max_retries: 3,
            retry_backoff_millis: 300,
        },
        storage: StorageConfig {
            backend: StorageBackend::Sqlite,
            sqlite_path: db_path.to_string_lossy().into_owned(),
            postgres_url: None,
        },
        includes: ConfigIncludes::default(),
        mimir: MimirConfig::default(),
        providers: vec![ProviderConfig {
            name: "infisical_main".to_string(),
            kind: ProviderKind::Infisical(InfisicalProviderConfig {
                api_base_url: "https://app.infisical.com".to_string(),
                client_id: "x".to_string(),
                client_secret: "y".to_string(),
                webhook_secret: "top-secret".to_string(),
                login_path: "/api/v1/auth/universal-auth/login".to_string(),
                secrets_path: "/api/v3/secrets/raw".to_string(),
            }),
        }],
        services: vec![ServiceConfig {
            name: "override-policy".to_string(),
            provider: "infisical_main".to_string(),
            secret_selector: SecretSelector {
                environment: "prod".to_string(),
                secret_path: "/papra".to_string(),
                include_keys: Vec::new(),
            },
            output: OutputConfig::TemplatedYaml {
                file_path: out_file.to_string_lossy().into_owned(),
                template: "value: {env:RATATOSKR_OVERRIDE_ENV}\n".to_string(),
                file_mode: None,
            },
            lifecycle: LifecycleAction::NoAction,
            security_profile: "strict".to_string(),
            placeholder_policy_override: Some(PlaceholderOverride {
                env: Some(true),
                file: None,
            }),
        }],
        security_profiles,
    };

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("RATATOSKR_OVERRIDE_ENV", "from-override") };
    let provider = Arc::new(MockProvider {
        calls: Arc::new(AtomicUsize::new(0)),
    });
    let mut providers = HashMap::<String, Arc<dyn ProviderClient>>::new();
    providers.insert("infisical_main".to_string(), provider);
    let store: Arc<dyn IdempotencyStore> = Arc::new(
        SqliteIdempotencyStore::new(&cfg.storage.sqlite_path)
            .await
            .unwrap(),
    );
    let engine = DispatchEngine::new(cfg, providers, store);

    let body =
        Bytes::from(r#"{"event":"secrets.modified","environment":"prod","secretPath":"/papra"}"#);
    let headers = signed_headers("top-secret", &body);
    engine
        .process_webhook("infisical_main", &headers, &body)
        .await
        .unwrap();

    let rendered = std::fs::read_to_string(out_file).unwrap();
    assert!(rendered.contains("from-override"));
}
