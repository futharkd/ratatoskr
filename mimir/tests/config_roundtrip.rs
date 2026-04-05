use std::path::PathBuf;

use mimir::config::{MimirConfig, load_toml_file};

#[test]
fn loads_minimal_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mimir_minimal.toml");
    let cfg: MimirConfig = load_toml_file(&path).unwrap();
    assert_eq!(cfg.placeholders.env, Some(true));
    assert_eq!(cfg.placeholders.file, Some(false));
    let policy = cfg.placeholder_policy();
    assert!(policy.allow_env_placeholders);
    assert!(!policy.allow_file_placeholders);
}

#[test]
fn loads_standalone_example_file() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/mimir.standalone.toml");
    let cfg: MimirConfig = load_toml_file(&path).unwrap();
    assert_eq!(cfg.placeholders.env, Some(false));
    assert_eq!(cfg.placeholders.file, Some(false));
}

#[test]
fn loads_nested_mimir_from_consumer_style_example() {
    #[derive(serde::Deserialize)]
    struct Root {
        mimir: MimirConfig,
    }
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/consumer.schema.example.toml");
    let root: Root = load_toml_file(&path).unwrap();
    assert_eq!(root.mimir.placeholders.env, Some(true));
    assert_eq!(root.mimir.placeholders.file, Some(false));
}
