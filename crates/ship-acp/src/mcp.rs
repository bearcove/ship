use std::io;
use std::path::Path;

use agent_client_protocol::{
    EnvVariable, HttpHeader, McpServer, McpServerHttp, McpServerSse, McpServerStdio,
};
use fs_err::tokio as fs;
use ship_types::{
    McpEnvVar, McpHeader, McpHttpServerConfig, McpServerConfig, McpSseServerConfig,
    McpStdioServerConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfigError {
    pub message: String,
}

impl std::fmt::Display for McpConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for McpConfigError {}

// r[acp.mcp.config]
// r[acp.mcp.defaults]
// r[project.mcp-defaults]
pub async fn resolve_mcp_servers(
    config_dir: &Path,
    project_root: &Path,
    session_override: Option<Vec<McpServerConfig>>,
) -> Result<Vec<McpServerConfig>, McpConfigError> {
    if let Some(mcp_servers) = session_override {
        return Ok(mcp_servers);
    }

    let project_defaults =
        load_mcp_servers_file(&project_root.join(".ship/mcp-servers.json")).await?;
    if let Some(mcp_servers) = project_defaults {
        return Ok(mcp_servers);
    }

    Ok(load_mcp_servers_file(&config_dir.join("mcp-servers.json"))
        .await?
        .unwrap_or_default())
}

pub fn to_acp_mcp_server(mcp_server: &McpServerConfig) -> McpServer {
    match mcp_server {
        McpServerConfig::Http(config) => McpServer::Http(
            McpServerHttp::new(config.name.clone(), config.url.clone())
                .headers(to_acp_headers(&config.headers)),
        ),
        McpServerConfig::Sse(config) => McpServer::Sse(
            McpServerSse::new(config.name.clone(), config.url.clone())
                .headers(to_acp_headers(&config.headers)),
        ),
        McpServerConfig::Stdio(config) => McpServer::Stdio(
            McpServerStdio::new(config.name.clone(), config.command.clone())
                .args(config.args.clone())
                .env(to_acp_env_vars(&config.env)),
        ),
    }
}

fn to_ship_mcp_server(mcp_server: McpServer) -> McpServerConfig {
    match mcp_server {
        McpServer::Http(config) => McpServerConfig::Http(McpHttpServerConfig {
            name: config.name,
            url: config.url,
            headers: config.headers.into_iter().map(to_ship_header).collect(),
        }),
        McpServer::Sse(config) => McpServerConfig::Sse(McpSseServerConfig {
            name: config.name,
            url: config.url,
            headers: config.headers.into_iter().map(to_ship_header).collect(),
        }),
        McpServer::Stdio(config) => McpServerConfig::Stdio(McpStdioServerConfig {
            name: config.name,
            command: config.command.to_string_lossy().into_owned(),
            args: config.args,
            env: config.env.into_iter().map(to_ship_env_var).collect(),
        }),
        _ => unreachable!("unsupported MCP server transport"),
    }
}

fn to_acp_headers(headers: &[McpHeader]) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|header| HttpHeader::new(header.name.clone(), header.value.clone()))
        .collect()
}

fn to_ship_header(header: HttpHeader) -> McpHeader {
    McpHeader {
        name: header.name,
        value: header.value,
    }
}

fn to_acp_env_vars(env_vars: &[McpEnvVar]) -> Vec<EnvVariable> {
    env_vars
        .iter()
        .map(|env_var| EnvVariable::new(env_var.name.clone(), env_var.value.clone()))
        .collect()
}

fn to_ship_env_var(env_var: EnvVariable) -> McpEnvVar {
    McpEnvVar {
        name: env_var.name,
        value: env_var.value,
    }
}

async fn load_mcp_servers_file(
    path: &Path,
) -> Result<Option<Vec<McpServerConfig>>, McpConfigError> {
    let bytes = match fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(McpConfigError {
                message: format!("failed to read {}: {error}", path.display()),
            });
        }
    };

    let mcp_servers =
        serde_json::from_slice::<Vec<McpServer>>(&bytes).map_err(|error| McpConfigError {
            message: format!("failed to parse {}: {error}", path.display()),
        })?;

    Ok(Some(
        mcp_servers.into_iter().map(to_ship_mcp_server).collect(),
    ))
}
