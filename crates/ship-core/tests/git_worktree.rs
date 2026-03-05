use std::path::{Path, PathBuf};
use std::process::Command;

use ship_core::{GitWorktreeOps, WorktreeOps};
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

// r[verify testability.git-trait]
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
    let session_id = SessionId("01J00000000000000000000000".to_owned());
    let slug = "real-worktree";
    let branch_name = format!("ship/{}/{slug}", &session_id.0[..8]);

    let worktree_path = ops
        .create_worktree(&session_id, "main", slug, &repo)
        .await
        .expect("create_worktree should succeed");

    assert!(worktree_path.exists(), "worktree path should exist");

    let branches = ops
        .list_branches(&repo)
        .await
        .expect("list_branches should succeed");
    assert!(branches.iter().any(|branch| branch == "main"));
    assert!(branches.iter().any(|branch| branch == &branch_name));

    let dirty_before = ops
        .has_uncommitted_changes(&worktree_path)
        .await
        .expect("status should work");
    assert!(!dirty_before);

    std::fs::write(worktree_path.join("scratch.txt"), "dirty\n").expect("should write file");
    let dirty_after = ops
        .has_uncommitted_changes(&worktree_path)
        .await
        .expect("status should work");
    assert!(dirty_after);

    ops.remove_worktree(&worktree_path)
        .await
        .expect("remove should succeed");
    assert!(!worktree_path.exists(), "worktree path should be removed");

    ops.delete_branch(&branch_name, false, &repo)
        .await
        .expect("delete branch should succeed");
    let branches_after = ops
        .list_branches(&repo)
        .await
        .expect("list_branches should succeed");
    assert!(!branches_after.iter().any(|branch| branch == &branch_name));

    let _ = std::fs::remove_dir_all(&root);
}
