use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::config::{Config, ProjectConfig};
use crate::state::State;

pub fn resolve(
    explicit: Option<&str>,
    config: &Config,
    state: &State,
) -> Result<ProjectConfig> {
    if let Some(name) = explicit {
        return config
            .find_project(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no project named '{}' — run `wt project add`", name));
    }

    let cwd = std::env::current_dir()?;

    if let Some(p) = config.find_project_by_path(&cwd) {
        return Ok(p.clone());
    }

    if let Some(entry) = state.worktrees.iter().find(|e| cwd.starts_with(&e.path)) {
        return config
            .find_project(&entry.project)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "worktree's project '{}' is no longer registered",
                    entry.project
                )
            });
    }

    bail!(
        "could not detect project from {} — pass --project or run `wt project add`",
        cwd.display()
    );
}

pub fn sanitize_branch(branch: &str) -> String {
    branch.replace('/', "-")
}

pub fn worktree_path(root: &std::path::Path, project: &str, branch: &str) -> PathBuf {
    root.join(project).join(sanitize_branch(branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_simple() {
        assert_eq!(sanitize_branch("main"), "main");
    }

    #[test]
    fn sanitize_slashes() {
        assert_eq!(sanitize_branch("feat/foo/bar"), "feat-foo-bar");
    }

    #[test]
    fn worktree_path_construction() {
        let root = std::path::Path::new("/coding/.worktrees");
        let result = worktree_path(root, "myapp", "feat/abc");
        assert_eq!(result, PathBuf::from("/coding/.worktrees/myapp/feat-abc"));
    }
}
