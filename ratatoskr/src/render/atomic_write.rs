use std::{fs, io::Write, os::unix::fs::PermissionsExt, path::Path};

use anyhow::Context;
use tempfile::NamedTempFile;

pub(super) fn atomic_write(
    path: &Path,
    content: &str,
    file_mode: Option<u32>,
) -> anyhow::Result<()> {
    let parent = path.parent().context("target path has no parent")?;
    let mut tmp_file = NamedTempFile::new_in(parent)?;
    tmp_file
        .write_all(content.as_bytes())
        .with_context(|| format!("failed writing temporary file for {}", path.display()))?;
    if let Some(mode) = file_mode {
        let perms = fs::Permissions::from_mode(mode);
        tmp_file.as_file().set_permissions(perms)?;
    }
    tmp_file
        .persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("failed persisting {}", path.display()))?;
    Ok(())
}
