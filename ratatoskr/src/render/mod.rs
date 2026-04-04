use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::Context;
use tempfile::NamedTempFile;

use crate::config::OutputConfig;

pub fn render_and_write(
    output: &OutputConfig,
    secrets: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    match output {
        OutputConfig::FlatFiles {
            directory,
            file_mode,
        } => write_flat_files(directory, *file_mode, secrets),
        OutputConfig::TemplatedYaml {
            file_path,
            template,
            file_mode,
        } => write_templated_yaml(file_path, template, *file_mode, secrets),
    }
}

fn write_flat_files(
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

fn write_templated_yaml(
    file_path: &str,
    template: &str,
    file_mode: Option<u32>,
    secrets: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    let mut rendered = template.to_string();
    for (key, value) in secrets {
        let marker = format!("{{{{{key}}}}}");
        rendered = rendered.replace(&marker, value);
    }
    let path = PathBuf::from(file_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(&path, &rendered, file_mode)
}

fn atomic_write(path: &Path, content: &str, file_mode: Option<u32>) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
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
}
