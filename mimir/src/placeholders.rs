use std::{env, fs, path::Path};

use anyhow::anyhow;

use crate::config::{PlaceholderPolicyOverride, SecurityProfileConfig, ServiceConfig};

#[derive(Debug, Clone, Copy, Default)]
pub struct PlaceholderPolicy {
    pub allow_env_placeholders: bool,
    pub allow_file_placeholders: bool,
}

pub fn resolve_placeholders(input: &str, policy: PlaceholderPolicy) -> anyhow::Result<String> {
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;

    while let Some(open_rel) = input[index..].find('{') {
        let open = index + open_rel;
        out.push_str(&input[index..open]);

        let Some(end_rel) = input[open..].find('}') else {
            return Err(anyhow!("unterminated placeholder"));
        };
        let end = open + end_rel;
        let token = &input[open + 1..end];

        if let Some(rest) = token.strip_prefix("env:") {
            validate_env_name(rest)?;
            if !policy.allow_env_placeholders {
                return Err(anyhow!(
                    "env placeholders are disabled by policy: {{env:...}}"
                ));
            }
            let value =
                env::var(rest).map_err(|_| anyhow!("environment variable not found: {rest}"))?;
            out.push_str(&value);
            index = end + 1;
            continue;
        }

        if let Some(path_str) = token.strip_prefix("file:") {
            if !policy.allow_file_placeholders {
                return Err(anyhow!(
                    "file placeholders are disabled by policy: {{file:...}}"
                ));
            }
            let path = Path::new(path_str);
            if !path.is_absolute() {
                return Err(anyhow!(
                    "file placeholder path must be absolute: {path_str}"
                ));
            }
            let value = fs::read_to_string(path)
                .map_err(|err| anyhow!("failed to read placeholder file `{path_str}`: {err}"))?;
            out.push_str(&value);
            index = end + 1;
            continue;
        }

        if token.starts_with('$') {
            return Err(anyhow!(
                "legacy env placeholder syntax is not supported; use {{env:VAR_NAME}}"
            ));
        }

        if looks_like_placeholder_token(token) {
            return Err(anyhow!("unsupported placeholder format: {{{token}}}"));
        }

        out.push('{');
        index = open + 1;
    }

    out.push_str(&input[index..]);
    Ok(out)
}

pub fn effective_placeholder_policy(
    profile: Option<&SecurityProfileConfig>,
    service: &ServiceConfig,
) -> PlaceholderPolicy {
    let base = PlaceholderPolicy {
        allow_env_placeholders: profile.map(|p| p.placeholders.env).unwrap_or(false),
        allow_file_placeholders: profile.map(|p| p.placeholders.file).unwrap_or(false),
    };
    apply_placeholder_override(base, service.placeholder_policy_override.as_ref())
}

fn apply_placeholder_override(
    base: PlaceholderPolicy,
    override_cfg: Option<&PlaceholderPolicyOverride>,
) -> PlaceholderPolicy {
    if let Some(override_cfg) = override_cfg {
        PlaceholderPolicy {
            allow_env_placeholders: override_cfg.env.unwrap_or(base.allow_env_placeholders),
            allow_file_placeholders: override_cfg.file.unwrap_or(base.allow_file_placeholders),
        }
    } else {
        base
    }
}

fn validate_env_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        return Err(anyhow!("empty environment placeholder name"));
    }
    if name
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return Ok(());
    }
    Err(anyhow!("invalid environment placeholder name: {name}"))
}

fn looks_like_placeholder_token(token: &str) -> bool {
    if token.is_empty() || token.contains(char::is_whitespace) {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '/' || c == '.')
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::config::{
        LifecycleAction, OutputConfig, ProfilePlaceholderPolicy, SecretSelector,
        SecurityProfileConfig, ServiceConfig,
    };

    use super::{PlaceholderPolicy, effective_placeholder_policy, resolve_placeholders};

    #[test]
    fn resolves_env_placeholder() {
        unsafe { std::env::set_var("MIMIR_TEST_ENV", "alpha") };
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: false,
        };
        let out = resolve_placeholders("token={env:MIMIR_TEST_ENV}", policy).unwrap();
        assert_eq!(out, "token=alpha");
    }

    #[test]
    fn resolves_file_placeholder() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        fs::write(&file_path, "super-secret").unwrap();
        let policy = PlaceholderPolicy {
            allow_env_placeholders: false,
            allow_file_placeholders: true,
        };
        let input = format!("key={{file:{}}}", file_path.to_string_lossy());
        let out = resolve_placeholders(&input, policy).unwrap();
        assert_eq!(out, "key=super-secret");
    }

    #[test]
    fn rejects_legacy_env_syntax() {
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: false,
        };
        let err = resolve_placeholders("{$NOPE}", policy).unwrap_err();
        assert!(err.to_string().contains("legacy env placeholder syntax"));
    }

    #[test]
    fn rejects_unknown_placeholder_format() {
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: true,
        };
        let err = resolve_placeholders("{vault:FOO}", policy).unwrap_err();
        assert!(err.to_string().contains("unsupported placeholder format"));
    }

    #[test]
    fn policy_helper_applies_service_override() {
        let profile = SecurityProfileConfig {
            allow_env_vars: false,
            require_signature: true,
            replay_tolerance_seconds: Some(300),
            placeholders: ProfilePlaceholderPolicy {
                env: false,
                file: false,
            },
        };
        let service = ServiceConfig {
            name: "svc".to_string(),
            provider: "infisical_main".to_string(),
            secret_selector: SecretSelector {
                environment: "prod".to_string(),
                secret_path: "/".to_string(),
                include_keys: vec![],
            },
            output: OutputConfig::FlatFiles {
                directory: "/tmp/x".to_string(),
                file_mode: None,
            },
            lifecycle: LifecycleAction::NoAction,
            security_profile: "strict".to_string(),
            placeholder_policy_override: Some(crate::config::PlaceholderPolicyOverride {
                env: Some(true),
                file: None,
            }),
        };
        let policy = effective_placeholder_policy(Some(&profile), &service);
        assert!(policy.allow_env_placeholders);
        assert!(!policy.allow_file_placeholders);
    }
}
