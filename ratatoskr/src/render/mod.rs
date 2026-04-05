mod atomic_write;
mod flat_files;
mod templated_yaml;

use std::collections::BTreeMap;

use crate::config::OutputConfig;

pub fn render_and_write(
    output: &OutputConfig,
    secrets: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    match output {
        OutputConfig::FlatFiles {
            directory,
            file_mode,
        } => flat_files::write_flat_files(directory, *file_mode, secrets),
        OutputConfig::TemplatedYaml {
            file_path,
            template,
            file_mode,
        } => templated_yaml::write_templated_yaml(file_path, template, *file_mode, secrets),
    }
}

#[cfg(test)]
mod tests;
