use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::Context;
use glob::glob;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConfigIncludes {
    #[serde(default)]
    pub providers: Vec<String>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub security_profiles: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct IncludeFiles {
    pub(crate) provider_files: Vec<PathBuf>,
    pub(crate) service_files: Vec<PathBuf>,
    pub(crate) profile_files: Vec<PathBuf>,
}

/// Resolves only globs listed in `[includes]` (no implicit convention paths).
pub(crate) fn resolve_includes(
    base_dir: &Path,
    includes: &ConfigIncludes,
) -> anyhow::Result<IncludeFiles> {
    Ok(IncludeFiles {
        provider_files: expand_globs(base_dir, &includes.providers)?,
        service_files: expand_globs(base_dir, &includes.services)?,
        profile_files: expand_globs(base_dir, &includes.security_profiles)?,
    })
}

fn expand_globs(base_dir: &Path, patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    for pattern in patterns {
        let full_pattern = if Path::new(pattern).is_absolute() {
            pattern.clone()
        } else {
            base_dir.join(pattern).to_string_lossy().to_string()
        };
        files.extend(expand_glob(&full_pattern)?);
    }
    Ok(files.into_iter().collect())
}

fn expand_glob(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    for entry in glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))? {
        let path =
            entry.with_context(|| format!("glob expansion failed for pattern: {pattern}"))?;
        if path.is_file() {
            results.push(path);
        }
    }
    Ok(results)
}
