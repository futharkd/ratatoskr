use std::fs;

use crate::config::OutputConfig;
use tempfile::tempdir;

use super::{PlaceholderPolicy, render_and_write};

#[test]
fn renders_flat_files() {
    let temp = tempdir().unwrap();
    let output = OutputConfig::FlatFiles {
        directory: temp.path().to_string_lossy().into_owned(),
        file_mode: None,
    };
    let secrets = std::collections::BTreeMap::from([
        ("API_KEY".to_string(), "alpha".to_string()),
        ("TOKEN".to_string(), "beta".to_string()),
    ]);

    render_and_write(&output, &secrets, PlaceholderPolicy::default()).unwrap();

    let api_key = fs::read_to_string(temp.path().join("api_key")).unwrap();
    let token = fs::read_to_string(temp.path().join("token")).unwrap();
    assert_eq!(api_key, "alpha");
    assert_eq!(token, "beta");
}

#[test]
fn resolves_placeholders_when_policy_allows() {
    let temp = tempdir().unwrap();
    let secret_file = temp.path().join("token.txt");
    fs::write(&secret_file, "from-file").unwrap();

    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("RATATOSKR_RENDER_ENV", "from-env") };
    let output = OutputConfig::TemplatedYaml {
        file_path: temp.path().join("out.yaml").to_string_lossy().into_owned(),
        template: "env: {env:RATATOSKR_RENDER_ENV}\nfile: {file:/tmp/placeholder}\n".to_string(),
        file_mode: None,
    };
    let fixed_output = if let OutputConfig::TemplatedYaml {
        file_path,
        template,
        file_mode,
    } = output
    {
        OutputConfig::TemplatedYaml {
            file_path,
            template: template.replace(
                "{file:/tmp/placeholder}",
                &format!("{{file:{}}}", secret_file.to_string_lossy()),
            ),
            file_mode,
        }
    } else {
        unreachable!()
    };
    let secrets = std::collections::BTreeMap::new();
    let policy = PlaceholderPolicy {
        allow_env_placeholders: true,
        allow_file_placeholders: true,
    };

    render_and_write(&fixed_output, &secrets, policy).unwrap();
    let out = fs::read_to_string(temp.path().join("out.yaml")).unwrap();
    assert!(out.contains("env: from-env"));
    assert!(out.contains("file: from-file"));
}
