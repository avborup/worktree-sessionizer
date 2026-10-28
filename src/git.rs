use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// git checks for submodules from inside its clean-worktree check, so it
/// refuses a worktree with checked-out submodules before ever looking at
/// whether that worktree is dirty. `--force` skips the whole check, submodules
/// included, so we run git's own cleanliness check ourselves and retry past it.
const SUBMODULE_BLOCK: &str = "working trees containing submodules cannot be moved or removed";

fn run_worktree_remove(repo: &Path, worktree: &Path, force: bool) -> Result<Output> {
    Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(worktree_remove_args(worktree, force))
        .output()
        .context("running git worktree remove")
}

/// The same status git runs inside its clean-worktree check: empty means the
/// worktree, submodules included, has nothing worth keeping.
fn worktree_status(worktree: &Path) -> Result<String> {
    let out = Command::new("git")
        .args(["-C", &worktree.to_string_lossy()])
        .args(["status", "--porcelain", "--ignore-submodules=none"])
        .output()
        .context("running git status")?;
    if !out.status.success() {
        bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // Only the trailing newline: porcelain's leading status column is meaningful.
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn worktree_remove(repo: &Path, worktree: &Path, force: bool) -> Result<()> {
    let out = run_worktree_remove(repo, worktree, force)?;
    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if force || !stderr.contains(SUBMODULE_BLOCK) {
        bail!("git worktree remove failed: {}", stderr);
    }

    let status = worktree_status(worktree)?;
    if !status.is_empty() {
        bail!(
            "worktree at {} contains modified or untracked files, use --force to remove it:\n{}",
            worktree.display(),
            status
        );
    }

    let out = run_worktree_remove(repo, worktree, true)?;
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

pub fn worktree_list(repo: &Path) -> Result<Vec<PathBuf>> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy()])
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("listing worktrees")?;
    if !out.status.success() {
        bail!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(parse_worktree_list(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_worktree_list(porcelain: &str) -> Vec<PathBuf> {
    porcelain
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    use super::{parse_worktree_list, worktree_remove, worktree_remove_args};

    #[test]
    fn parse_worktree_list_reads_paths_in_order() {
        let porcelain = "worktree /repo\nHEAD abc\nbranch refs/heads/main\n\n\
                         worktree /wt/feat x\nHEAD def\ndetached\nprunable gitdir file points to non-existent location\n";

        assert_eq!(
            parse_worktree_list(porcelain),
            vec![PathBuf::from("/repo"), PathBuf::from("/wt/feat x")]
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(["-C", &dir.to_string_lossy()])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn commit(dir: &Path, message: &str) {
        git(
            dir,
            &[
                "-c",
                "user.email=wt@test",
                "-c",
                "user.name=wt",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    /// A superproject holding one submodule, plus room to hang worktrees off it.
    struct Fixture {
        _root: TempDir,
        root: PathBuf,
        repo: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root_dir = tempfile::tempdir().expect("tempdir");
            let root = root_dir.path().to_path_buf();
            let sub = root.join("sub");
            let repo = root.join("super");
            std::fs::create_dir(&sub).unwrap();
            std::fs::create_dir(&repo).unwrap();

            git(&sub, &["init", "-q", "-b", "main"]);
            std::fs::write(sub.join("a.txt"), "hi").unwrap();
            git(&sub, &["add", "-A"]);
            commit(&sub, "init");

            git(&repo, &["init", "-q", "-b", "main"]);
            std::fs::write(repo.join("top.txt"), "top").unwrap();
            git(&repo, &["add", "-A"]);
            commit(&repo, "init");
            git(
                &repo,
                &[
                    "-c",
                    "protocol.file.allow=always",
                    "submodule",
                    "add",
                    "-q",
                    "../sub",
                    "vendor/sub",
                ],
            );
            commit(&repo, "add submodule");

            Self {
                _root: root_dir,
                root,
                repo,
            }
        }

        /// A worktree whose submodule is checked out, which is what makes git
        /// refuse to remove it.
        fn worktree(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            git(
                &self.repo,
                &["worktree", "add", "-q", "-b", name, &path.to_string_lossy()],
            );
            git(
                &path,
                &[
                    "-c",
                    "protocol.file.allow=always",
                    "submodule",
                    "update",
                    "--init",
                    "-q",
                ],
            );
            assert!(
                path.join("vendor/sub/a.txt").exists(),
                "submodule checked out"
            );
            path
        }
    }

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

    #[test]
    fn removes_clean_worktree_containing_submodules() {
        let fixture = Fixture::new();
        let worktree = fixture.worktree("clean");

        worktree_remove(&fixture.repo, &worktree, false).expect("removing clean worktree");

        assert!(!worktree.exists(), "worktree directory removed");
        assert!(
            !fixture.repo.join(".git/worktrees/clean").exists(),
            "git bookkeeping removed too, submodule gitdir included"
        );
    }

    #[test]
    fn refuses_dirty_worktree_containing_submodules() {
        let fixture = Fixture::new();
        let worktree = fixture.worktree("dirty");
        std::fs::write(worktree.join("scratch.txt"), "untracked").unwrap();

        let err = worktree_remove(&fixture.repo, &worktree, false)
            .expect_err("dirty worktree must not be removed");

        assert!(
            err.to_string().contains("--force"),
            "error points at --force: {}",
            err
        );
        assert!(worktree.exists(), "worktree left in place");
    }

    #[test]
    fn refuses_worktree_with_dirty_submodule() {
        let fixture = Fixture::new();
        let worktree = fixture.worktree("dirty-sub");
        std::fs::write(worktree.join("vendor/sub/a.txt"), "changed").unwrap();

        let err = worktree_remove(&fixture.repo, &worktree, false)
            .expect_err("dirty submodule must not be removed");

        assert!(
            err.to_string().contains("--force"),
            "error points at --force: {}",
            err
        );
        assert!(worktree.exists(), "worktree left in place");
    }

    #[test]
    fn force_removes_dirty_worktree_containing_submodules() {
        let fixture = Fixture::new();
        let worktree = fixture.worktree("forced");
        std::fs::write(worktree.join("scratch.txt"), "untracked").unwrap();

        worktree_remove(&fixture.repo, &worktree, true).expect("force removing worktree");

        assert!(!worktree.exists(), "worktree directory removed");
    }
}
