use std::path::{Path, PathBuf};

use fs_err::tokio as fs;
use ship_core::ProjectRegistry;
use ulid::Ulid;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("ship-project-registry-test-{}", Ulid::new()))
}

async fn mk_repo(root: &Path, name: &str) -> PathBuf {
    let repo = root.join(name);
    fs::create_dir_all(repo.join(".git"))
        .await
        .expect("should create fake git dir");
    repo
}

// r[verify project.registration]
// r[verify project.identity]
#[tokio::test]
async fn add_list_duplicate_and_remove() {
    let root = test_root();
    let config_dir = root.join("config");
    fs::create_dir_all(&root)
        .await
        .expect("should create test root");

    let repo_a = mk_repo(&root, "alpha").await;
    let duplicate_parent = root.join("nested");
    fs::create_dir_all(&duplicate_parent)
        .await
        .expect("should create nested parent");
    let repo_b = mk_repo(&duplicate_parent, "alpha").await;

    let mut registry = ProjectRegistry::load_in(config_dir)
        .await
        .expect("should load registry");

    let first = registry.add(&repo_a).await.expect("first add should work");
    assert_eq!(first.name.0, "alpha");

    let second = registry.add(&repo_b).await.expect("second add should work");
    assert_eq!(second.name.0, "alpha-2");

    let listed = registry.list();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|project| project.name.0 == "alpha"));
    assert!(listed.iter().any(|project| project.name.0 == "alpha-2"));

    let removed = registry.remove("alpha").await.expect("remove should work");
    assert!(removed);
    assert!(registry.get("alpha").is_none());
    assert!(registry.get("alpha-2").is_some());

    fs::remove_dir_all(&root)
        .await
        .expect("should clean test root");
}

// r[verify project.validation]
#[tokio::test]
async fn validate_marks_missing_paths_invalid() {
    let root = test_root();
    let config_dir = root.join("config");
    fs::create_dir_all(&root)
        .await
        .expect("should create test root");
    let repo = mk_repo(&root, "beta").await;

    let mut registry = ProjectRegistry::load_in(config_dir)
        .await
        .expect("should load registry");
    let added = registry.add(&repo).await.expect("add should work");
    assert!(added.valid);

    fs::remove_dir_all(&repo).await.expect("should remove repo");
    registry
        .validate_all()
        .await
        .expect("validation should run");

    let invalid = registry
        .get("beta")
        .expect("project should still be registered after validation");
    assert!(!invalid.valid);
    assert_eq!(
        invalid.invalid_reason.as_deref(),
        Some("path does not exist")
    );

    fs::remove_dir_all(&root)
        .await
        .expect("should clean test root");
}
