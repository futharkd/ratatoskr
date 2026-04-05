use std::{collections::BTreeMap, fs, path::PathBuf};

use super::atomic_write::atomic_write;

pub(super) fn write_flat_files(
    directory: &str,
    file_mode: Option<u32>,
    secrets: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    fs::create_dir_all(directory)?;
    for (key, value) in secrets {
        let target = PathBuf::from(directory).join(key.to_lowercase());
        atomic_write(&target, value, file_mode)?;
    }
    Ok(())
}
