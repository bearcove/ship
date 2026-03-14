use std::io;
use std::path::Path;

use fs_err::tokio as fs;
use ship_types::{ProjectConfig, ResolvedHooks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfigError {
    pub message: String,
}

impl std::fmt::Display for ProjectConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ProjectConfigError {}

/// Load project hooks from `.config/ship/config.styx` at the project root.
/// Returns empty/default hooks if the config file doesn't exist.
pub async fn load_project_hooks(project_root: &Path) -> Result<ResolvedHooks, ProjectConfigError> {
    let config_path = project_root.join(".config/ship/config.styx");
    let source = match fs::read_to_string(&config_path).await {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ResolvedHooks::default());
        }
        Err(error) => {
            return Err(ProjectConfigError {
                message: format!("failed to read {}: {error}", config_path.display()),
            });
        }
    };

    let config =
        facet_styx::from_str::<ProjectConfig>(&source).map_err(|error| ProjectConfigError {
            message: format!("failed to parse {}: {error}", config_path.display()),
        })?;

    Ok(config.hooks.resolve())
}
