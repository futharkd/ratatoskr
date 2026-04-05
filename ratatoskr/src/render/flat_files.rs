use std::{collections::BTreeMap, fs, path::PathBuf};

use super::atomic_write::atomic_write;
use crate::placeholders::{PlaceholderPolicy, resolve_placeholders};

pub(super) fn write_flat_files(
    directory: &str,
    file_mode: Option<u32>,
    secrets: &BTreeMap<String, String>,
    placeholder_policy: PlaceholderPolicy,
) -> anyhow::Result<()> {
    fs::create_dir_all(directory)?;
    for (key, value) in secrets {
        let target = PathBuf::from(directory).join(key.to_lowercase());
        let rendered = resolve_placeholders(value, placeholder_policy)?;
        atomic_write(&target, &rendered, file_mode)?;
    }
    Ok(())
}
