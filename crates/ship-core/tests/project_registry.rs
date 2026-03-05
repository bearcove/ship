use std::fs;
use std::path::PathBuf;

use ship_core::ProjectRegistry;
use ulid::Ulid;

fn test_root() -> PathBuf {
    std::env::temp_dir().join(format!("ship-project-registry-test-{}", Ulid::new()))
}

fn mk_repo(root: &PathBuf, name: &str) -> PathBuf {
    let repo = root.join(name);
    fs::create_dir_all(repo.join(".git")).expect("should create fake git dir");
    repo
}

// r[verify project.registration]
// r[verify project.identity]
#[test]
fn add_list_duplicate_and_remove() {
    let root = test_root();
    let config_dir = root.join("config");
    fs::create_dir_all(&root).expect("should create test root");

    let repo_a = mk_repo(&root, "alpha");
    let duplicate_parent = root.join("nested");
    fs::create_dir_all(&duplicate_parent).expect("should create nested parent");
    let repo_b = mk_repo(&duplicate_parent, "alpha");

    let mut registry = ProjectRegistry::load_in(config_dir).expect("should load registry");

    let first = registry.add(&repo_a).expect("first add should work");
    assert_eq!(first.name.0, "alpha");

    let second = registry.add(&repo_b).expect("second add should work");
    assert_eq!(second.name.0, "alpha-2");

    let listed = registry.list();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|project| project.name.0 == "alpha"));
    assert!(listed.iter().any(|project| project.name.0 == "alpha-2"));

    let removed = registry.remove("alpha").expect("remove should work");
    assert!(removed);
    assert!(registry.get("alpha").is_none());
    assert!(registry.get("alpha-2").is_some());

    fs::remove_dir_all(&root).expect("should clean test root");
}

// r[verify project.validation]
#[test]
fn validate_marks_missing_paths_invalid() {
    let root = test_root();
    let config_dir = root.join("config");
    fs::create_dir_all(&root).expect("should create test root");
    let repo = mk_repo(&root, "beta");

    let mut registry = ProjectRegistry::load_in(config_dir).expect("should load registry");
    let added = registry.add(&repo).expect("add should work");
    assert!(added.valid);

    fs::remove_dir_all(&repo).expect("should remove repo");
    registry.validate_all().expect("validation should run");

    let invalid = registry
        .get("beta")
        .expect("project should still be registered after validation");
    assert!(!invalid.valid);
    assert_eq!(
        invalid.invalid_reason.as_deref(),
        Some("path does not exist")
    );

    fs::remove_dir_all(&root).expect("should clean test root");
}
