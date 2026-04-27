use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn worktree_add(repo: &Path, target: &Path, branch: &str) -> Result<()> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["worktree", "add", &target.to_string_lossy(), branch])
        .output()
        .context("running git worktree add")?;
    if !out.status.success() {
        bail!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn worktree_add_new_branch(
    repo: &Path,
    target: &Path,
    new_branch: &str,
    base: &str,
) -> Result<()> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args([
            "worktree",
            "add",
            "-b",
            new_branch,
            &target.to_string_lossy(),
            base,
        ])
        .output()
        .context("running git worktree add -b")?;
    if !out.status.success() {
        bail!(
            "git worktree add -b failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn unset_upstream(worktree: &Path) -> Result<()> {
    let _ = Command::new("git")
        .args(["-C", &worktree.to_string_lossy()])
        .args(["branch", "--unset-upstream"])
        .output();
    Ok(())
}

pub fn worktree_remove(repo: &Path, worktree: &Path) -> Result<()> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["worktree", "remove", &worktree.to_string_lossy()])
        .output()
        .context("running git worktree remove")?;
    if !out.status.success() {
        bail!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn branch_delete(repo: &Path, branch: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["branch", flag, branch])
        .output()
        .context("running git branch delete")?;
    if !out.status.success() {
        bail!(
            "git branch {} failed: {}",
            flag,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn current_branch(worktree: &Path) -> Result<String> {
    let out = Command::new("git")
        .args(["-C", &worktree.to_string_lossy()])
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("detecting branch")?;
    if !out.status.success() {
        bail!(
            "not a git worktree: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn is_branch_merged(repo: &Path, branch: &str, into: &str) -> Result<bool> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["branch", "--merged", into])
        .output()
        .context("checking merged branches")?;
    if !out.status.success() {
        return Ok(false);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout.lines().any(|line| line.trim().trim_start_matches("* ") == branch))
}
