use log::{info, trace, warn};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

static ACTIVE_WORKTREES: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();

fn active_worktrees() -> &'static Mutex<Vec<PathBuf>> {
    ACTIVE_WORKTREES.get_or_init(|| Mutex::new(Vec::new()))
}

fn locked_active_worktrees() -> std::sync::MutexGuard<'static, Vec<PathBuf>> {
    match active_worktrees().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn cleanup_active_worktrees() {
    let mut guard = locked_active_worktrees();
    for path in guard.drain(..) {
        force_remove_worktree(&path, None);
    }
}

fn force_remove_worktree(worktree_path: &Path, repo: Option<&Path>) {
    let mut cmd = Command::new("git");
    cmd.args(["worktree", "remove", "--force"])
        .arg(worktree_path);
    if let Some(repo) = repo {
        cmd.current_dir(repo);
    }
    let _ = cmd.status();
    if worktree_path.exists() {
        let _ = std::fs::remove_dir_all(worktree_path);
    }
}

pub fn ensure_clean_tree(repo: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("failed to run `git status` in {}: {}", repo.display(), e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`git status` failed in {} (not a git repo, or git unavailable): {}",
            repo.display(),
            stderr.trim()
        ));
    }
    if !output.stdout.is_empty() {
        return Err(format!(
            "Working tree at {} has uncommitted changes. Parallel runs create worktrees from HEAD and would not include uncommitted work. Commit or stash, then retry.",
            repo.display()
        ));
    }
    Ok(())
}

pub struct Worktree {
    path: PathBuf,
    repo: PathBuf,
    cleanup_on_drop: bool,
}

impl Worktree {
    pub fn create(repo: &Path, name: &str) -> Result<Worktree, String> {
        let base =
            std::env::temp_dir().join(format!("transmute-worker-{}-{}", std::process::id(), name));
        if base.exists() {
            force_remove_worktree(&base, Some(repo));
        }

        trace!(
            "creating worktree {} (from {})",
            base.display(),
            repo.display()
        );
        let status = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&base)
            .arg("HEAD")
            .current_dir(repo)
            .status()
            .map_err(|e| format!("failed to spawn `git worktree add`: {}", e))?;
        if !status.success() {
            return Err(format!(
                "`git worktree add` exited with status {} for {}",
                status,
                base.display()
            ));
        }

        locked_active_worktrees().push(base.clone());

        Ok(Worktree {
            path: base,
            repo: repo.to_path_buf(),
            cleanup_on_drop: true,
        })
    }

    pub fn run_setup(&self, command: &str) -> Result<(), String> {
        info!("running setup in {}: {}", self.path.display(), command);
        let parts = shlex::split(command)
            .ok_or_else(|| format!("could not parse --setup-command: {}", command))?;
        if parts.is_empty() {
            return Err("--setup-command was empty after shell parsing".to_string());
        }
        let status = Command::new(&parts[0])
            .args(&parts[1..])
            .current_dir(&self.path)
            .status()
            .map_err(|e| format!("failed to spawn setup command '{}': {}", parts[0], e))?;
        if !status.success() {
            return Err(format!(
                "setup command exited with status {} in {}",
                status,
                self.path.display()
            ));
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn keep_on_failure(&mut self) {
        self.cleanup_on_drop = false;
        warn!(
            "preserving worktree {} for inspection (cleanup disabled)",
            self.path.display()
        );
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        locked_active_worktrees().retain(|p| p != &self.path);
        if !self.cleanup_on_drop {
            return;
        }
        force_remove_worktree(&self.path, Some(&self.repo));
    }
}
