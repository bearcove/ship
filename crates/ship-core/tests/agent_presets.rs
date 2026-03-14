use std::path::{Path, PathBuf};

use ship_core::load_agent_presets;
use ship_types::{AgentKind, AgentPreset, AgentPresetId, AgentProviderId};

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn write_config(path: &Path, config: &str) {
    std::fs::write(path, config).expect("config should be written");
}

// r[verify server.config-dir]
#[tokio::test]
async fn loads_agent_presets_from_global_config_styx() {
    let root = make_temp_dir("agent-presets-valid");
    let config_dir = root.join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir should exist");
    write_config(
        &config_dir.join("config.styx"),
        r#"
agent_presets {
    presets (
        {id claude::sonnet, label "Claude Sonnet", kind @Claude, provider anthropic, model_id claude-sonnet-4}
        {id codex::gpt-5.4, label "GPT 5.4", kind @Codex, provider openai, model_id gpt-5.4}
    )
}
"#,
    );

    let presets = load_agent_presets(&config_dir)
        .await
        .expect("config should parse");

    assert_eq!(
        presets,
        vec![
            AgentPreset {
                id: AgentPresetId("claude::sonnet".to_owned()),
                label: "Claude Sonnet".to_owned(),
                kind: AgentKind::Claude,
                provider: AgentProviderId("anthropic".to_owned()),
                model_id: "claude-sonnet-4".to_owned(),
            },
            AgentPreset {
                id: AgentPresetId("codex::gpt-5.4".to_owned()),
                label: "GPT 5.4".to_owned(),
                kind: AgentKind::Codex,
                provider: AgentProviderId("openai".to_owned()),
                model_id: "gpt-5.4".to_owned(),
            },
        ]
    );

    let _ = std::fs::remove_dir_all(root);
}

// r[verify server.config-dir]
#[tokio::test]
async fn reports_invalid_agent_preset_config_with_file_context() {
    let root = make_temp_dir("agent-presets-invalid");
    let config_dir = root.join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir should exist");
    let config_path = config_dir.join("config.styx");
    write_config(
        &config_path,
        r#"
agent_presets {
    presets (
        {id claude::sonnet, kind @Claude, provider anthropic, model_id claude-sonnet-4}
    )
}
"#,
    );

    let error = load_agent_presets(&config_dir)
        .await
        .expect_err("invalid config should fail");

    assert!(error.message.contains("failed to parse"));
    assert!(error.message.contains(&config_path.display().to_string()));

    let _ = std::fs::remove_dir_all(root);
}

// r[verify server.config-dir]
#[tokio::test]
async fn missing_global_config_returns_no_agent_presets() {
    let root = make_temp_dir("agent-presets-missing");
    let config_dir = root.join("config");

    let presets = load_agent_presets(&config_dir)
        .await
        .expect("missing config should be allowed");

    assert!(presets.is_empty());

    let _ = std::fs::remove_dir_all(root);
}
