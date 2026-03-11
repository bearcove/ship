use std::path::{Path, PathBuf};
use std::process::Command;

use ship_core::{GitWorktreeOps, SessionGitNames, WorktreeOps};
use ship_types::SessionId;

fn run_git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command should start");

    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn make_temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-core-{test_name}-{}", ulid::Ulid::new()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

// r[verify backend.git-shell]
// r[verify testability.git-trait]
// r[verify worktree.path]
// r[verify worktree.branch-name]
#[tokio::test]
async fn git_worktree_create_status_and_remove() {
    let root = make_temp_dir("git-worktree");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("repo dir should be created");

    run_git(&["init"], &repo);
    run_git(&["config", "user.name", "Ship Test"], &repo);
    run_git(&["config", "user.email", "ship@example.com"], &repo);
    std::fs::write(repo.join("README.md"), "seed\n").expect("seed file should write");
    run_git(&["add", "README.md"], &repo);
    run_git(&["commit", "-m", "seed"], &repo);
    run_git(&["branch", "-M", "main"], &repo);

    let ops = GitWorktreeOps;
    let clean_session_id = SessionId("01J0000000ABCD000000000000".to_owned());
    let clean_names = SessionGitNames::from_session_id(&clean_session_id);

    assert_eq!(clean_names.branch_name, "ship-abcd");
    assert_eq!(clean_names.worktree_dir, "@abcd");

    let clean_worktree_path = ops
        .create_worktree(
            &clean_names.branch_name,
            &clean_names.worktree_dir,
            "main",
            &repo,
        )
        .await
        .expect("create_worktree should succeed");

    assert!(clean_worktree_path.exists(), "worktree path should exist");
    assert_eq!(clean_worktree_path, repo.join(".ship").join("@abcd"));

    let branches = ops
        .list_branches(&repo)
        .await
        .expect("list_branches should succeed");
    assert!(branches.iter().any(|branch| branch == "main"));
    assert!(
        branches
            .iter()
            .any(|branch| branch == &clean_names.branch_name)
    );

    let clean_dirty = ops
        .has_uncommitted_changes(&clean_worktree_path)
        .await
        .expect("status should work");
    assert!(!clean_dirty);

    ops.remove_worktree(&clean_worktree_path, false)
        .await
        .expect("clean remove should succeed");
    assert!(
        !clean_worktree_path.exists(),
        "clean worktree path should be removed"
    );

    ops.delete_branch(&clean_names.branch_name, false, &repo)
        .await
        .expect("delete branch should succeed");

    let dirty_session_id = SessionId("01J0000000WXYZ000000000000".to_owned());
    let dirty_names = SessionGitNames::from_session_id(&dirty_session_id);

    assert_eq!(dirty_names.branch_name, "ship-wxyz");
    assert_eq!(dirty_names.worktree_dir, "@wxyz");

    let worktree_path = ops
        .create_worktree(
            &dirty_names.branch_name,
            &dirty_names.worktree_dir,
            "main",
            &repo,
        )
        .await
        .expect("dirty create_worktree should succeed");

    assert_eq!(worktree_path, repo.join(".ship").join("@wxyz"));

    let dirty_after = ops
        .has_uncommitted_changes(&worktree_path)
        .await
        .expect("status should work");
    assert!(!dirty_after);

    std::fs::write(worktree_path.join("scratch.txt"), "dirty\n").expect("should write file");
    let dirty_after_write = ops
        .has_uncommitted_changes(&worktree_path)
        .await
        .expect("status should work");
    assert!(dirty_after_write);

    let remove_without_force = ops.remove_worktree(&worktree_path, false).await;
    assert!(
        remove_without_force.is_err(),
        "dirty worktree removal should require force"
    );
    assert!(
        worktree_path.exists(),
        "dirty worktree should remain on disk"
    );

    ops.remove_worktree(&worktree_path, true)
        .await
        .expect("forced remove should succeed");
    assert!(!worktree_path.exists(), "worktree path should be removed");

    ops.delete_branch(&dirty_names.branch_name, true, &repo)
        .await
        .expect("delete branch should succeed");
    let branches_after = ops
        .list_branches(&repo)
        .await
        .expect("list_branches should succeed");
    assert!(
        !branches_after
            .iter()
            .any(|branch| branch == &clean_names.branch_name)
    );
    assert!(
        !branches_after
            .iter()
            .any(|branch| branch == &dirty_names.branch_name)
    );

    let _ = std::fs::remove_dir_all(&root);
}

// r[verify worktree.base-branch]
#[tokio::test]
async fn git_worktree_rejects_invalid_or_unborn_base_branch() {
    let root = make_temp_dir("git-worktree-invalid-base");
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("repo dir should be created");

    run_git(&["init"], &repo);
    run_git(&["config", "user.name", "Ship Test"], &repo);
    run_git(&["config", "user.email", "ship@example.com"], &repo);
    run_git(&["checkout", "-b", "main"], &repo);

    let ops = GitWorktreeOps;
    let unborn_names =
        SessionGitNames::from_session_id(&SessionId("01J0000000UNBR000000000000".to_owned()));
    let unborn_error = ops
        .create_worktree(
            &unborn_names.branch_name,
            &unborn_names.worktree_dir,
            "main",
            &repo,
        )
        .await
        .expect_err("unborn branch should be rejected");
    assert!(
        unborn_error.message.contains("is unborn"),
        "unexpected unborn error: {}",
        unborn_error.message
    );

    std::fs::write(repo.join("README.md"), "seed\n").expect("seed file should write");
    run_git(&["add", "README.md"], &repo);
    run_git(&["commit", "-m", "seed"], &repo);

    let invalid_names =
        SessionGitNames::from_session_id(&SessionId("01J0000000MISS000000000000".to_owned()));
    let invalid_error = ops
        .create_worktree(
            &invalid_names.branch_name,
            &invalid_names.worktree_dir,
            "missing-branch",
            &repo,
        )
        .await
        .expect_err("invalid branch should be rejected");
    assert!(
        invalid_error
            .message
            .contains("does not resolve to a commit"),
        "unexpected invalid error: {}",
        invalid_error.message
    );

    let _ = std::fs::remove_dir_all(&root);
}
