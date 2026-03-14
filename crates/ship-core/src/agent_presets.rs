use std::io;
use std::path::Path;

use facet::Facet;
use fs_err::tokio as fs;
use ship_types::{AgentPreset, AgentPresetsConfig};

const CONFIG_FILE_NAME: &str = "config.styx";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPresetConfigError {
    pub message: String,
}

impl std::fmt::Display for AgentPresetConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AgentPresetConfigError {}

#[derive(Debug, Default, Facet)]
struct ShipConfigFile {
    #[facet(default)]
    agent_presets: AgentPresetsConfig,
}

pub async fn load_agent_presets(
    config_dir: &Path,
) -> Result<Vec<AgentPreset>, AgentPresetConfigError> {
    let config_path = config_dir.join(CONFIG_FILE_NAME);
    let source = match fs::read_to_string(&config_path).await {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // Missing config is valid: presets are optional server-level config.
            return Ok(Vec::new());
        }
        Err(error) => {
            return Err(AgentPresetConfigError {
                message: format!("failed to read {}: {error}", config_path.display()),
            });
        }
    };

    let config = facet_styx::from_str::<ShipConfigFile>(&source).map_err(|error| {
        AgentPresetConfigError {
            message: format!("failed to parse {}: {error}", config_path.display()),
        }
    })?;

    Ok(config.agent_presets.presets)
}
