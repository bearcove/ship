use std::path::PathBuf;

use ship_core::resolve_mcp_servers;
use ship_types::{McpServerConfig, McpStdioServerConfig};

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn stdio_server(name: &str, command: &str) -> McpServerConfig {
    McpServerConfig::Stdio(McpStdioServerConfig {
        name: name.to_owned(),
        command: command.to_owned(),
        args: Vec::new(),
        env: Vec::new(),
    })
}

fn write_mcp_servers(path: &std::path::Path, servers: &str) {
    std::fs::write(path, servers).expect("mcp config should be written");
}

// r[verify acp.mcp.defaults]
// r[verify project.mcp-defaults]
#[tokio::test]
async fn project_defaults_override_global_defaults() {
    let root = make_temp_dir("project-defaults");
    let config_dir = root.join("config");
    let project_root = root.join("project");
    std::fs::create_dir_all(config_dir.clone()).expect("config dir should exist");
    std::fs::create_dir_all(project_root.join(".ship")).expect("project ship dir should exist");

    write_mcp_servers(
        &config_dir.join("mcp-servers.json"),
        r#"[{"name":"global","command":"/usr/bin/global-mcp","args":[],"env":[]}]"#,
    );
    write_mcp_servers(
        &project_root.join(".ship/mcp-servers.json"),
        r#"[{"name":"project","command":"/usr/bin/project-mcp","args":[],"env":[]}]"#,
    );

    let resolved = resolve_mcp_servers(&config_dir, &project_root, None)
        .await
        .expect("mcp defaults should resolve");

    assert_eq!(
        resolved,
        vec![stdio_server("project", "/usr/bin/project-mcp")]
    );

    let _ = std::fs::remove_dir_all(root);
}

// r[verify acp.mcp.config]
#[tokio::test]
async fn session_override_beats_backend_defaults() {
    let root = make_temp_dir("session-override");
    let config_dir = root.join("config");
    let project_root = root.join("project");
    std::fs::create_dir_all(config_dir.clone()).expect("config dir should exist");
    std::fs::create_dir_all(project_root.join(".ship")).expect("project ship dir should exist");

    write_mcp_servers(
        &config_dir.join("mcp-servers.json"),
        r#"[{"name":"global","command":"/usr/bin/global-mcp","args":[],"env":[]}]"#,
    );

    let resolved = resolve_mcp_servers(
        &config_dir,
        &project_root,
        Some(vec![stdio_server("override", "/usr/bin/override-mcp")]),
    )
    .await
    .expect("session override should resolve");

    assert_eq!(
        resolved,
        vec![stdio_server("override", "/usr/bin/override-mcp")]
    );

    let _ = std::fs::remove_dir_all(root);
}
