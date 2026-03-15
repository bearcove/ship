use camino::Utf8PathBuf;

use super::*;

/// Create a temp dir as a Utf8PathBuf.
fn temp_dir() -> Utf8PathBuf {
    let dir = std::env::temp_dir().join(format!("ship-git-test-{}", std::process::id()));
    Utf8PathBuf::try_from(dir).expect("temp dir is not valid UTF-8")
}

/// Set up a fresh git repo with a single initial commit.
async fn setup_repo() -> (GitContext, Utf8PathBuf) {
    let base = temp_dir();
    let dir = base.join(format!("{}", rand_u64()));
    let ctx = GitContext::init(&dir, &BranchName::new("main"))
        .await
        .expect("init");
    ctx.config_set("user.email", "test@test.com").await.unwrap();
    ctx.config_set("user.name", "Test").await.unwrap();

    // Create an initial commit so HEAD exists
    let file = dir.join("README.md");
    tokio::fs::write(file.as_std_path(), "# hello\n")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("initial commit").await.unwrap();

    (ctx, dir)
}

fn rand_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

async fn cleanup(dir: &Utf8Path) {
    let _ = tokio::fs::remove_dir_all(dir.as_std_path()).await;
}

#[tokio::test]
async fn test_init_and_branch_name() {
    let (ctx, dir) = setup_repo().await;
    let branch = ctx.branch_name().await.unwrap();
    assert_eq!(branch.as_str(), "main");
    cleanup(&dir).await;
}

#[tokio::test]
async fn test_commit_and_rev_parse() {
    let (ctx, dir) = setup_repo().await;

    tokio::fs::write(dir.join("file.txt").as_std_path(), "content")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    let info = ctx.commit("add file").await.unwrap();

    assert_eq!(info.subject, "add file");
    assert!(!info.hash.as_str().is_empty());

    let head = ctx.rev_parse(&Rev::new("HEAD")).await.unwrap();
    assert_eq!(head.as_str(), info.hash.as_str());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_status() {
    let (ctx, dir) = setup_repo().await;

    // Clean status after commit
    let status = ctx.status().await.unwrap();
    assert!(status.is_clean());

    // Create untracked file
    tokio::fs::write(dir.join("new.txt").as_std_path(), "new")
        .await
        .unwrap();
    let status = ctx.status().await.unwrap();
    assert!(!status.is_clean());
    assert!(status.has_untracked());

    // Stage it
    ctx.add_all().await.unwrap();
    let status = ctx.status().await.unwrap();
    assert!(status.has_staged());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_diff_operations() {
    let (ctx, dir) = setup_repo().await;

    let base_hash = ctx.rev_parse(&Rev::new("HEAD")).await.unwrap();
    let base_rev = Rev::from(&base_hash);

    tokio::fs::write(dir.join("a.txt").as_std_path(), "hello\n")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("add a").await.unwrap();

    tokio::fs::write(dir.join("b.txt").as_std_path(), "world\n")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    let head_info = ctx.commit("add b").await.unwrap();

    let head_rev = Rev::from(&head_info.hash);

    // Diff between base and head should show changes
    let diff = ctx.diff(&base_rev, &head_rev).await.unwrap();
    assert!(diff.as_str().contains("a.txt"));

    // Numstat
    let stats = ctx.diff_numstat(&base_rev, &head_rev).await.unwrap();
    assert_eq!(stats.files_changed(), 2);
    assert!(stats.total_added() > 0);

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_log() {
    let (ctx, dir) = setup_repo().await;

    tokio::fs::write(dir.join("x.txt").as_std_path(), "x")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("second commit").await.unwrap();

    let entries = ctx.log("HEAD~1..HEAD").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].subject, "second commit");

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_checkout_new_branch() {
    let (ctx, dir) = setup_repo().await;

    ctx.checkout_new_branch(&BranchName::new("feature"))
        .await
        .unwrap();
    let branch = ctx.branch_name().await.unwrap();
    assert_eq!(branch.as_str(), "feature");

    // Switch back
    ctx.checkout(&BranchName::new("main")).await.unwrap();
    let branch = ctx.branch_name().await.unwrap();
    assert_eq!(branch.as_str(), "main");

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_branch_list_and_delete() {
    let (ctx, dir) = setup_repo().await;

    ctx.checkout_new_branch(&BranchName::new("to-delete"))
        .await
        .unwrap();
    ctx.checkout(&BranchName::new("main")).await.unwrap();

    let branches = ctx.branch_list().await.unwrap();
    assert!(branches.iter().any(|b| b.as_str() == "to-delete"));

    ctx.branch_delete(&BranchName::new("to-delete"), false)
        .await
        .unwrap();
    let branches = ctx.branch_list().await.unwrap();
    assert!(!branches.iter().any(|b| b.as_str() == "to-delete"));

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_reset_hard() {
    let (ctx, dir) = setup_repo().await;

    let before = ctx.rev_parse(&Rev::new("HEAD")).await.unwrap();

    tokio::fs::write(dir.join("reset-me.txt").as_std_path(), "data")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("to be reset").await.unwrap();

    ctx.reset_hard(&Rev::from(&before)).await.unwrap();
    let after = ctx.rev_parse(&Rev::new("HEAD")).await.unwrap();
    assert_eq!(before.as_str(), after.as_str());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_show_file() {
    let (ctx, dir) = setup_repo().await;

    tokio::fs::write(dir.join("show-me.txt").as_std_path(), "the content")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("add show-me").await.unwrap();

    let content = ctx
        .show_file(&Rev::new("HEAD"), Utf8Path::new("show-me.txt"))
        .await
        .unwrap();
    assert_eq!(content, "the content");

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_ls_files() {
    let (ctx, dir) = setup_repo().await;

    let files = ctx.ls_files().await.unwrap();
    assert!(files.iter().any(|f| f.as_str() == "README.md"));

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_rev_list_count() {
    let (ctx, dir) = setup_repo().await;

    let count = ctx.rev_list_count(&Rev::new("HEAD")).await.unwrap();
    assert_eq!(count, 1);

    tokio::fs::write(dir.join("c.txt").as_std_path(), "c")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("second").await.unwrap();

    let count = ctx.rev_list_count(&Rev::new("HEAD")).await.unwrap();
    assert_eq!(count, 2);

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_ref_exists() {
    let (ctx, dir) = setup_repo().await;

    assert!(ctx.ref_exists(&Rev::new("HEAD")).await.unwrap());
    assert!(!ctx.ref_exists(&Rev::new("nonexistent-ref")).await.unwrap());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_merge_base() {
    let (ctx, dir) = setup_repo().await;

    let base_hash = ctx.rev_parse(&Rev::new("HEAD")).await.unwrap();

    ctx.checkout_new_branch(&BranchName::new("feature-mb"))
        .await
        .unwrap();
    tokio::fs::write(dir.join("feat.txt").as_std_path(), "feat")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("feature commit").await.unwrap();

    let mb = ctx
        .merge_base(&Rev::new("main"), &Rev::new("feature-mb"))
        .await
        .unwrap();
    assert_eq!(mb.as_str(), base_hash.as_str());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_diff_cached_quiet() {
    let (ctx, dir) = setup_repo().await;

    // Nothing staged
    assert!(!ctx.diff_cached_quiet().await.unwrap());

    // Stage something
    tokio::fs::write(dir.join("staged.txt").as_std_path(), "data")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    assert!(ctx.diff_cached_quiet().await.unwrap());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_stash() {
    let (ctx, dir) = setup_repo().await;

    tokio::fs::write(dir.join("stash-me.txt").as_std_path(), "stash data")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();

    ctx.stash_push().await.unwrap();
    let status = ctx.status().await.unwrap();
    assert!(status.is_clean());

    ctx.stash_pop().await.unwrap();
    let status = ctx.status().await.unwrap();
    assert!(!status.is_clean());

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_rebase_success() {
    let (ctx, dir) = setup_repo().await;

    // Create a commit on main
    tokio::fs::write(dir.join("main-file.txt").as_std_path(), "main")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("main commit").await.unwrap();

    // Branch off, add a commit
    ctx.checkout_new_branch(&BranchName::new("rebase-test"))
        .await
        .unwrap();
    tokio::fs::write(dir.join("feature-file.txt").as_std_path(), "feature")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("feature commit").await.unwrap();

    // Rebase onto main (should be a no-op since we branched from latest main)
    let outcome = ctx.rebase(&Rev::new("main")).await.unwrap();
    assert!(matches!(outcome, RebaseOutcome::Success));

    cleanup(&dir).await;
}

#[tokio::test]
async fn test_show_and_show_numstat() {
    let (ctx, dir) = setup_repo().await;

    tokio::fs::write(dir.join("show-test.txt").as_std_path(), "line1\nline2\n")
        .await
        .unwrap();
    ctx.add_all().await.unwrap();
    ctx.commit("for show").await.unwrap();

    let diff = ctx.show(&Rev::new("HEAD")).await.unwrap();
    assert!(diff.as_str().contains("show-test.txt"));

    let stats = ctx.show_numstat(&Rev::new("HEAD")).await.unwrap();
    assert_eq!(stats.files_changed(), 1);

    cleanup(&dir).await;
}
