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

fn worktree_remove_args(worktree: &Path, force: bool) -> Vec<String> {
    let mut args = vec!["worktree".to_string(), "remove".to_string()];
    if force {
        args.push("--force".to_string());
    }
    args.push(worktree.to_string_lossy().into_owned());
    args
}

pub fn worktree_remove(repo: &Path, worktree: &Path, force: bool) -> Result<()> {
    let args = worktree_remove_args(worktree, force);
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(&args)
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::worktree_remove_args;

    #[test]
    fn worktree_remove_args_omit_force_by_default() {
        assert_eq!(
            worktree_remove_args(Path::new("/worktree"), false),
            vec!["worktree", "remove", "/worktree"]
        );
    }

    #[test]
    fn worktree_remove_args_include_force_when_requested() {
        assert_eq!(
            worktree_remove_args(Path::new("/worktree"), true),
            vec!["worktree", "remove", "--force", "/worktree"]
        );
    }
}
