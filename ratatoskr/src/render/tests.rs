use std::fs;

use tempfile::tempdir;

use crate::config::OutputConfig;

use super::render_and_write;

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

    render_and_write(&output, &secrets).unwrap();

    let api_key = fs::read_to_string(temp.path().join("api_key")).unwrap();
    let token = fs::read_to_string(temp.path().join("token")).unwrap();
    assert_eq!(api_key, "alpha");
    assert_eq!(token, "beta");
}
