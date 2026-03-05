use std::env;
use std::ffi::OsString;
use std::path::Path;

use ship_types::AgentDiscovery;

const CLAUDE_AGENT_BINARY: &str = "claude-agent-acp";
const CODEX_AGENT_BINARY: &str = "codex-acp";

pub trait BinaryPathProbe {
    fn is_available(&self, binary: &str) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemBinaryPathProbe;

impl BinaryPathProbe for SystemBinaryPathProbe {
    fn is_available(&self, binary: &str) -> bool {
        is_binary_available_on_path(binary, env::var_os("PATH"))
    }
}

// r[server.agent-discovery]
pub fn discover_agents(probe: &impl BinaryPathProbe) -> AgentDiscovery {
    AgentDiscovery {
        claude: probe.is_available(CLAUDE_AGENT_BINARY),
        codex: probe.is_available(CODEX_AGENT_BINARY),
    }
}

fn is_binary_available_on_path(binary: &str, path_var: Option<OsString>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };

    env::split_paths(&path_var).any(|dir| is_executable_file(&dir.join(binary)))
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ship_types::AgentDiscovery;

    use super::{discover_agents, is_binary_available_on_path};

    fn make_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ship-agent-discovery-{test_name}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    fn write_fake_binary(dir: &std::path::Path, name: &str) {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").expect("fake binary should be written");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&path)
                .expect("fake binary metadata should exist")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions)
                .expect("fake binary permissions should be updated");
        }
    }

    struct FakeProbe {
        claude: bool,
        codex: bool,
    }

    impl super::BinaryPathProbe for FakeProbe {
        fn is_available(&self, binary: &str) -> bool {
            match binary {
                "claude-agent-acp" => self.claude,
                "codex-acp" => self.codex,
                other => panic!("unexpected binary lookup: {other}"),
            }
        }
    }

    // r[verify server.agent-discovery]
    #[test]
    fn path_probe_requires_the_binary_to_exist_on_path() {
        let dir = make_temp_dir("path-probe");
        write_fake_binary(&dir, "claude-agent-acp");

        let path_var = std::env::join_paths([dir.as_path()]).expect("path should join");

        assert!(is_binary_available_on_path(
            "claude-agent-acp",
            Some(path_var.clone())
        ));
        assert!(!is_binary_available_on_path("codex-acp", Some(path_var)));

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify server.agent-discovery]
    #[test]
    fn discovery_reports_each_agent_kind_independently() {
        let discovery = discover_agents(&FakeProbe {
            claude: false,
            codex: true,
        });

        assert_eq!(
            discovery,
            AgentDiscovery {
                claude: false,
                codex: true,
            }
        );
    }
}
