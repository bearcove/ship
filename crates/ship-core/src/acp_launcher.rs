use std::env;
use std::ffi::OsString;
use std::path::Path;

use ship_types::{AgentDiscovery, AgentKind};

const NPX_BINARY: &str = "pnpx";
const CLAUDE_AGENT_BINARY: &str = "claude-agent-acp";
const CODEX_AGENT_BINARY: &str = "codex-acp";
const CLAUDE_AGENT_NPX_PACKAGE: &str = "@zed-industries/claude-agent-acp";
const CODEX_AGENT_NPX_PACKAGE: &str = "@zed-industries/codex-acp";
const EMPTY_ARGS: &[&str] = &[];
const CLAUDE_AGENT_NPX_ARGS: &[&str] = &[CLAUDE_AGENT_NPX_PACKAGE];
const CODEX_AGENT_NPX_ARGS: &[&str] = &[CODEX_AGENT_NPX_PACKAGE];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentLauncher {
    pub program: &'static str,
    pub args: &'static [&'static str],
}

impl AgentLauncher {
    pub const fn new(program: &'static str, args: &'static [&'static str]) -> Self {
        Self { program, args }
    }
}

// r[server.agent-discovery]
pub fn discover_agents(probe: &impl BinaryPathProbe) -> AgentDiscovery {
    AgentDiscovery {
        claude: resolve_agent_launcher(AgentKind::Claude, probe).is_some(),
        codex: resolve_agent_launcher(AgentKind::Codex, probe).is_some(),
    }
}

// r[acp.binary.claude]
// r[acp.binary.codex]
pub fn resolve_agent_launcher(
    kind: AgentKind,
    probe: &impl BinaryPathProbe,
) -> Option<AgentLauncher> {
    let binary = match kind {
        AgentKind::Claude => CLAUDE_AGENT_BINARY,
        AgentKind::Codex => CODEX_AGENT_BINARY,
    };

    if probe.is_available(binary) {
        return Some(AgentLauncher::new(binary, EMPTY_ARGS));
    }

    if probe.is_available(NPX_BINARY) {
        let args = match kind {
            AgentKind::Claude => CLAUDE_AGENT_NPX_ARGS,
            AgentKind::Codex => CODEX_AGENT_NPX_ARGS,
        };
        return Some(AgentLauncher::new(NPX_BINARY, args));
    }

    None
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

    use ship_types::{AgentDiscovery, AgentKind};

    use super::{
        AgentLauncher, BinaryPathProbe, CLAUDE_AGENT_BINARY, CODEX_AGENT_BINARY, NPX_BINARY,
        discover_agents, is_binary_available_on_path, resolve_agent_launcher,
    };

    fn make_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ship-agent-launcher-{test_name}-{nanos}"));
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

    #[derive(Clone, Copy)]
    struct FakeProbe {
        claude: bool,
        codex: bool,
        pnpx: bool,
    }

    impl BinaryPathProbe for FakeProbe {
        fn is_available(&self, binary: &str) -> bool {
            match binary {
                CLAUDE_AGENT_BINARY => self.claude,
                CODEX_AGENT_BINARY => self.codex,
                NPX_BINARY => self.pnpx,
                other => panic!("unexpected binary lookup: {other}"),
            }
        }
    }

    // r[verify server.agent-discovery]
    #[test]
    fn path_probe_requires_the_binary_to_exist_on_path() {
        let dir = make_temp_dir("path-probe");
        write_fake_binary(&dir, CLAUDE_AGENT_BINARY);

        let path_var = std::env::join_paths([dir.as_path()]).expect("path should join");

        assert!(is_binary_available_on_path(
            CLAUDE_AGENT_BINARY,
            Some(path_var.clone())
        ));
        assert!(!is_binary_available_on_path(
            CODEX_AGENT_BINARY,
            Some(path_var)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    // r[verify server.agent-discovery]
    #[test]
    fn discovery_reports_agent_available_when_direct_binary_exists() {
        let discovery = discover_agents(&FakeProbe {
            claude: true,
            codex: false,
            pnpx: false,
        });

        assert_eq!(
            discovery,
            AgentDiscovery {
                claude: true,
                codex: false,
            }
        );
    }

    // r[verify server.agent-discovery]
    #[test]
    fn discovery_reports_agent_available_when_pnpx_fallback_exists() {
        let discovery = discover_agents(&FakeProbe {
            claude: false,
            codex: true,
            pnpx: true,
        });

        assert_eq!(
            discovery,
            AgentDiscovery {
                claude: true,
                codex: true,
            }
        );
    }

    // r[verify acp.binary.claude]
    #[test]
    fn claude_launcher_resolution_supports_direct_binary_and_pnpx_fallback() {
        assert_eq!(
            resolve_agent_launcher(
                AgentKind::Claude,
                &FakeProbe {
                    claude: true,
                    codex: false,
                    pnpx: true,
                }
            ),
            Some(AgentLauncher::new(CLAUDE_AGENT_BINARY, &[]))
        );
        assert_eq!(
            resolve_agent_launcher(
                AgentKind::Claude,
                &FakeProbe {
                    claude: false,
                    codex: false,
                    pnpx: true,
                }
            ),
            Some(AgentLauncher::new(
                NPX_BINARY,
                &["@zed-industries/claude-agent-acp"]
            ))
        );
    }

    // r[verify acp.binary.codex]
    #[test]
    fn codex_launcher_resolution_supports_direct_binary_and_pnpx_fallback() {
        assert_eq!(
            resolve_agent_launcher(
                AgentKind::Codex,
                &FakeProbe {
                    claude: false,
                    codex: true,
                    pnpx: true,
                }
            ),
            Some(AgentLauncher::new(CODEX_AGENT_BINARY, &[]))
        );
        assert_eq!(
            resolve_agent_launcher(
                AgentKind::Codex,
                &FakeProbe {
                    claude: false,
                    codex: false,
                    pnpx: true,
                }
            ),
            Some(AgentLauncher::new(
                NPX_BINARY,
                &["@zed-industries/codex-acp"]
            ))
        );
    }

    // r[verify server.agent-discovery]
    #[test]
    fn discovery_and_launcher_resolution_always_agree() {
        let scenarios = [
            FakeProbe {
                claude: false,
                codex: false,
                pnpx: false,
            },
            FakeProbe {
                claude: true,
                codex: false,
                pnpx: false,
            },
            FakeProbe {
                claude: false,
                codex: true,
                pnpx: false,
            },
            FakeProbe {
                claude: false,
                codex: false,
                pnpx: true,
            },
            FakeProbe {
                claude: true,
                codex: true,
                pnpx: true,
            },
        ];

        for probe in scenarios {
            let discovery = discover_agents(&probe);
            assert_eq!(
                discovery.claude,
                resolve_agent_launcher(AgentKind::Claude, &probe).is_some()
            );
            assert_eq!(
                discovery.codex,
                resolve_agent_launcher(AgentKind::Codex, &probe).is_some()
            );
        }
    }
}
