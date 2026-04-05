use std::{env, fs, path::Path};

use anyhow::anyhow;

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

        if input[open..].starts_with("{$") {
            let end_rel = input[open..]
                .find('}')
                .ok_or_else(|| anyhow!("unterminated env placeholder"))?;
            let end = open + end_rel;
            let placeholder = &input[open + 2..end];
            validate_env_name(placeholder)?;
            if !policy.allow_env_placeholders {
                return Err(anyhow!("env placeholders are disabled by policy: {{$...}}"));
            }
            let value = env::var(placeholder)
                .map_err(|_| anyhow!("environment variable not found: {placeholder}"))?;
            out.push_str(&value);
            index = end + 1;
            continue;
        }

        if input[open..].starts_with("{file:") {
            let end_rel = input[open..]
                .find('}')
                .ok_or_else(|| anyhow!("unterminated file placeholder"))?;
            let end = open + end_rel;
            let path_str = &input[open + 6..end];
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

        // Reject placeholder-looking tokens with unsupported format.
        if let Some(end_rel) = input[open..].find('}') {
            let end = open + end_rel;
            let token = &input[open + 1..end];
            if looks_like_placeholder_token(token) {
                return Err(anyhow!("unsupported placeholder format: {{{token}}}"));
            }
        }

        out.push('{');
        index = open + 1;
    }

    out.push_str(&input[index..]);
    Ok(out)
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
    token.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '$' || c == '/' || c == '.'
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{PlaceholderPolicy, resolve_placeholders};

    #[test]
    fn resolves_env_placeholder() {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("RATATOSKR_TEST_ENV", "alpha") };
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: false,
        };
        let out = resolve_placeholders("token={$RATATOSKR_TEST_ENV}", policy).unwrap();
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
    fn rejects_disallowed_env_placeholder() {
        let policy = PlaceholderPolicy::default();
        let err = resolve_placeholders("{$NOPE}", policy).unwrap_err();
        assert!(err.to_string().contains("env placeholders are disabled"));
    }

    #[test]
    fn rejects_unknown_placeholder_format() {
        let policy = PlaceholderPolicy {
            allow_env_placeholders: true,
            allow_file_placeholders: true,
        };
        let err = resolve_placeholders("{env:FOO}", policy).unwrap_err();
        assert!(err.to_string().contains("unsupported placeholder format"));
    }
}
