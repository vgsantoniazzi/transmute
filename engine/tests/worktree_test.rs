use std::path::{Path, PathBuf};
use std::process::Command;
use transmute::worktree;

fn init_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "transmute_worktree_test_{}_{}",
        std::process::id(),
        name
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).ok();
    }
    std::fs::create_dir_all(&dir).unwrap();

    git(&dir, &["init", "--quiet"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.join("README"), "seed\n").unwrap();
    git(&dir, &["add", "README"]);
    git(&dir, &["commit", "--quiet", "-m", "init"]);

    dir
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn test_ensure_clean_tree_accepts_clean_repo() {
    let repo = init_repo("clean");
    worktree::ensure_clean_tree(&repo).expect("clean repo must pass the check");
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_ensure_clean_tree_rejects_repo_with_untracked_file() {
    let repo = init_repo("untracked");
    std::fs::write(repo.join("scratch.txt"), "wip\n").unwrap();

    let err = worktree::ensure_clean_tree(&repo).unwrap_err();
    assert!(
        err.contains("uncommitted changes") && err.contains("Commit or stash"),
        "Error must call out uncommitted state and suggest the fix; got: {}",
        err
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_ensure_clean_tree_rejects_repo_with_staged_changes() {
    let repo = init_repo("staged");
    std::fs::write(repo.join("README"), "edited\n").unwrap();
    git(&repo, &["add", "README"]);

    let err = worktree::ensure_clean_tree(&repo).unwrap_err();
    assert!(err.contains("uncommitted changes"));

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_ensure_clean_tree_errors_when_not_a_git_repo() {
    let dir = std::env::temp_dir().join(format!(
        "transmute_worktree_test_{}_notrepo",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let err = worktree::ensure_clean_tree(&dir).unwrap_err();
    assert!(
        err.contains("not a git repo") || err.contains("`git status` failed"),
        "Non-repo must surface a git error; got: {}",
        err
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_worktree_create_produces_directory_with_source_files() {
    let repo = init_repo("create");
    let wt = worktree::Worktree::create(&repo, "create-test").expect("create must succeed");

    assert!(wt.path().exists(), "worktree path must exist on disk");
    assert!(
        wt.path().join("README").exists(),
        "worktree must contain HEAD source files"
    );

    drop(wt);
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_worktree_drop_removes_directory_by_default() {
    let repo = init_repo("drop_removes");
    let path;
    {
        let wt = worktree::Worktree::create(&repo, "drop-test").unwrap();
        path = wt.path().to_path_buf();
        assert!(path.exists());
    }
    assert!(
        !path.exists(),
        "Drop must remove the worktree directory by default"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_worktree_keep_on_failure_preserves_directory() {
    let repo = init_repo("keep");
    let path;
    {
        let mut wt = worktree::Worktree::create(&repo, "keep-test").unwrap();
        path = wt.path().to_path_buf();
        wt.keep_on_failure();
    }
    assert!(
        path.exists(),
        "keep_on_failure must prevent cleanup so the operator can inspect"
    );

    let _ = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(&path)
        .current_dir(&repo)
        .status();
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_run_setup_executes_command_inside_worktree() {
    let repo = init_repo("setup_ok");
    let wt = worktree::Worktree::create(&repo, "setup-ok").unwrap();
    wt.run_setup("sh -c 'touch sentinel'")
        .expect("setup must run inside the worktree");

    assert!(
        wt.path().join("sentinel").exists(),
        "setup command must have cwd set to the worktree"
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_run_setup_surfaces_command_failure() {
    let repo = init_repo("setup_fail");
    let wt = worktree::Worktree::create(&repo, "setup-fail").unwrap();
    let err = wt.run_setup("sh -c 'exit 7'").unwrap_err();
    assert!(
        err.contains("status") && err.contains("7"),
        "Setup failure must surface the non-zero exit; got: {}",
        err
    );

    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn test_run_setup_rejects_unparseable_command() {
    let repo = init_repo("setup_bad_parse");
    let wt = worktree::Worktree::create(&repo, "setup-bad-parse").unwrap();
    let err = wt.run_setup("'unterminated").unwrap_err();
    assert!(
        err.contains("could not parse"),
        "Unparseable command must produce a clear error; got: {}",
        err
    );

    std::fs::remove_dir_all(&repo).ok();
}
