use std::{collections::BTreeMap, fs, path::PathBuf};

use super::atomic_write::atomic_write;
use super::placeholders::{PlaceholderPolicy, resolve_placeholders};

pub(super) fn write_templated_yaml(
    file_path: &str,
    template: &str,
    file_mode: Option<u32>,
    secrets: &BTreeMap<String, String>,
    placeholder_policy: PlaceholderPolicy,
) -> anyhow::Result<()> {
    let mut rendered = template.to_string();
    for (key, value) in secrets {
        let marker = format!("{{{{{key}}}}}");
        rendered = rendered.replace(&marker, value);
    }
    let rendered = resolve_placeholders(&rendered, placeholder_policy)?;
    let path = PathBuf::from(file_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(&path, &rendered, file_mode)
}
