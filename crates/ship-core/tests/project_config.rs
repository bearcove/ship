use std::path::{Path, PathBuf};

use ship_core::load_project_hooks;
use ship_types::{HookDef, ResolvedHooks};

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn write_project_config(project_root: &Path, config: &str) {
    let config_dir = project_root.join(".config/ship");
    std::fs::create_dir_all(&config_dir).expect("config dir should be created");
    std::fs::write(config_dir.join("config.styx"), config).expect("config file should be written");
}

#[tokio::test]
async fn missing_project_config_returns_empty_hooks() {
    let root = make_temp_dir("project-hooks-missing");

    let hooks = load_project_hooks(&root)
        .await
        .expect("missing config should be allowed");

    assert_eq!(hooks, ResolvedHooks::default());
    assert!(hooks.worktree_setup.is_empty());
    assert!(hooks.pre_commit.is_empty());
    assert!(hooks.post_commit.is_empty());

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn loads_all_hook_types() {
    let root = make_temp_dir("project-hooks-all-types");
    write_project_config(
        &root,
        r#"
hooks {
    worktree_setup {
        pnpm-install {
            command "pnpm install"
        }
    }
    pre_commit {
        format-rust {
            command "cargo fmt"
        }
    }
    post_commit {
        clippy {
            command "cargo clippy --workspace"
        }
        typecheck {
            command "pnpm typecheck"
            cwd frontend
        }
    }
}
"#,
    );

    let hooks = load_project_hooks(&root)
        .await
        .expect("valid config should parse");

    assert_eq!(
        hooks.worktree_setup,
        vec![HookDef {
            name: "pnpm-install".to_owned(),
            command: "pnpm install".to_owned(),
            cwd: None,
        }]
    );

    assert_eq!(
        hooks.pre_commit,
        vec![HookDef {
            name: "format-rust".to_owned(),
            command: "cargo fmt".to_owned(),
            cwd: None,
        }]
    );

    // post_commit hooks should be sorted alphabetically: clippy before typecheck
    assert_eq!(
        hooks.post_commit,
        vec![
            HookDef {
                name: "clippy".to_owned(),
                command: "cargo clippy --workspace".to_owned(),
                cwd: None,
            },
            HookDef {
                name: "typecheck".to_owned(),
                command: "pnpm typecheck".to_owned(),
                cwd: Some("frontend".to_owned()),
            },
        ]
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cwd_is_optional_and_defaults_to_none() {
    let root = make_temp_dir("project-hooks-cwd");
    write_project_config(
        &root,
        r#"
hooks {
    post_commit {
        no-cwd {
            command "cargo test"
        }
        with-cwd {
            command "pnpm test"
            cwd frontend
        }
    }
}
"#,
    );

    let hooks = load_project_hooks(&root)
        .await
        .expect("valid config should parse");

    // sorted: no-cwd < with-cwd
    assert_eq!(hooks.post_commit[0].name, "no-cwd");
    assert_eq!(hooks.post_commit[0].cwd, None);

    assert_eq!(hooks.post_commit[1].name, "with-cwd");
    assert_eq!(hooks.post_commit[1].cwd, Some("frontend".to_owned()));

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn hooks_sorted_alphabetically_by_name() {
    let root = make_temp_dir("project-hooks-sort");
    write_project_config(
        &root,
        r#"
hooks {
    pre_commit {
        zzz-last {
            command "echo last"
        }
        aaa-first {
            command "echo first"
        }
        mmm-middle {
            command "echo middle"
        }
    }
}
"#,
    );

    let hooks = load_project_hooks(&root)
        .await
        .expect("valid config should parse");

    let names: Vec<&str> = hooks.pre_commit.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, vec!["aaa-first", "mmm-middle", "zzz-last"]);

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn partial_config_with_only_some_hook_types() {
    let root = make_temp_dir("project-hooks-partial");
    write_project_config(
        &root,
        r#"
hooks {
    post_commit {
        clippy {
            command "cargo clippy --workspace"
        }
    }
}
"#,
    );

    let hooks = load_project_hooks(&root)
        .await
        .expect("partial config should parse");

    assert!(hooks.worktree_setup.is_empty());
    assert!(hooks.pre_commit.is_empty());
    assert_eq!(hooks.post_commit.len(), 1);
    assert_eq!(hooks.post_commit[0].command, "cargo clippy --workspace");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn empty_hooks_block_is_valid() {
    let root = make_temp_dir("project-hooks-empty-block");
    write_project_config(
        &root,
        r#"
hooks {
}
"#,
    );

    let hooks = load_project_hooks(&root)
        .await
        .expect("empty hooks block should parse");

    assert_eq!(hooks, ResolvedHooks::default());

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn reports_invalid_config_with_file_context() {
    let root = make_temp_dir("project-hooks-invalid");
    let config_path = root.join(".config/ship/config.styx");
    write_project_config(
        &root,
        r#"
hooks {
    post_commit {
        clippy {
            this is not valid styx syntax !!!
        }
    }
}
"#,
    );

    let error = load_project_hooks(&root)
        .await
        .expect_err("invalid config should fail");

    assert!(
        error.message.contains("failed to parse"),
        "error should mention parse failure: {error:?}"
    );
    assert!(
        error.message.contains(&config_path.display().to_string()),
        "error should include config path: {error:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}
