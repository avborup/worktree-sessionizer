use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use crate::config::{Config, ProjectConfig};
use crate::state::{State, WorktreeEntry};
use crate::{clone, git, paths, project};

/// A wt data dir: `config.toml`, `state.toml` and the `state.lock` guarding it.
///
/// Every write goes through the lock, so the `wt` binary and other tools using
/// this library can share one data dir.
#[derive(Debug, Clone)]
pub struct Wt {
    home: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CreateWorktree {
    pub project: String,
    pub branch: String,
    /// Branch `branch` off this ref. `None` checks out an existing `branch`.
    pub base: Option<String>,
    pub session_name: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveOptions {
    pub delete_branch: bool,
    /// Discard worktree changes and untracked files; with `delete_branch`,
    /// delete the branch even if unmerged.
    pub force: bool,
}

/// What `create_with` and `remove_with` are about to do, for callers that
/// show progress.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Step<'a> {
    Branching { branch: &'a str, base: &'a str },
    CheckingOut { branch: &'a str },
    WorktreeAdded { path: &'a Path },
    CowCloning { entry: &'a str },
    Symlinking { entry: &'a str },
    RemovingWorktree { path: &'a Path },
    DeletingBranch { branch: &'a str, force: bool },
    SavingMetadata,
}

impl Wt {
    /// `$WT_HOME`, else the platform data dir the `wt` binary uses.
    pub fn from_env() -> Self {
        Self::at(paths::data_dir())
    }

    pub fn at(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn state_path(&self) -> PathBuf {
        paths::state_path(&self.home)
    }

    pub fn config(&self) -> Result<Config> {
        Config::load_in(&self.home)
    }

    /// A snapshot read without the lock; use `mutate` to change it.
    pub fn state(&self) -> Result<State> {
        State::load_in(&self.home)
    }

    pub fn mutate<T>(&self, f: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
        State::mutate_in(&self.home, f)
    }

    pub fn create(&self, req: &CreateWorktree) -> Result<WorktreeEntry> {
        self.create_with(req, |_| {})
    }

    /// Add the worktree, clone and link the project's files into it, and save
    /// its entry. If anything fails once git has added the worktree, the
    /// worktree (and the branch, if this call created it) is removed again.
    #[tracing::instrument(level = "info", skip_all, fields(project = %req.project, branch = %req.branch), name = "wt.create")]
    pub fn create_with(
        &self,
        req: &CreateWorktree,
        mut on_step: impl FnMut(Step),
    ) -> Result<WorktreeEntry> {
        let config = self.config()?;
        let proj = config
            .find_project(&req.project)
            .ok_or_else(|| anyhow!("no project named '{}' — run `wt project add`", req.project))?;

        let target = project::worktree_path(&config.worktrees_root, &proj.name, &req.branch);
        if target.exists() {
            bail!("worktree already exists at {}", target.display());
        }

        // A branch that only exists once `git worktree add` has run is this
        // call's to delete on rollback, whether `-b` made it or git made a
        // local branch tracking origin's.
        let branch_existed = git::branch_exists(&proj.repo_path, &req.branch)?;

        self.add_worktree(proj, req, &target, &mut on_step)
            .map_err(|err| {
                match rollback_create(&proj.repo_path, &target, &req.branch, branch_existed) {
                    Ok(()) => err,
                    Err(rollback) => anyhow!("{err:#} (rollback failed too: {rollback:#})"),
                }
            })
    }

    fn add_worktree(
        &self,
        proj: &ProjectConfig,
        req: &CreateWorktree,
        target: &Path,
        on_step: &mut impl FnMut(Step),
    ) -> Result<WorktreeEntry> {
        match &req.base {
            Some(base) => {
                on_step(Step::Branching {
                    branch: &req.branch,
                    base,
                });
                git::worktree_add_new_branch(&proj.repo_path, target, &req.branch, base)?;
            }
            None => {
                on_step(Step::CheckingOut {
                    branch: &req.branch,
                });
                git::worktree_add(&proj.repo_path, target, &req.branch)?;
            }
        }
        self.finish_create(proj, req, target, on_step)
    }

    fn finish_create(
        &self,
        proj: &ProjectConfig,
        req: &CreateWorktree,
        target: &Path,
        on_step: &mut impl FnMut(Step),
    ) -> Result<WorktreeEntry> {
        if req.base.is_some() {
            git::unset_upstream(target)?;
        }
        on_step(Step::WorktreeAdded { path: target });

        for entry in &proj.cow_clone {
            let src = proj.repo_path.join(entry);
            if src.exists() {
                on_step(Step::CowCloning { entry });
                clone::cow_clone(&src, &target.join(entry))?;
            }
        }

        for entry in &proj.symlink {
            let src = proj.repo_path.join(entry);
            if src.exists() {
                on_step(Step::Symlinking { entry });
                std::os::unix::fs::symlink(&src, target.join(entry))?;
            }
        }

        on_step(Step::SavingMetadata);
        let entry = WorktreeEntry::new(
            proj.name.clone(),
            req.branch.clone(),
            target.to_path_buf(),
            req.session_name.clone(),
            req.base.clone(),
        );
        self.mutate(|state| {
            state.add(entry.clone());
            Ok(())
        })?;
        Ok(entry)
    }

    pub fn remove(&self, id: &str, opts: RemoveOptions) -> Result<()> {
        self.remove_with(id, opts, |_| {})
    }

    /// Remove the worktree (and, if asked, its branch) and forget its entry.
    /// The entry is kept when git refuses, e.g. over a dirty worktree.
    #[tracing::instrument(level = "info", skip_all, fields(id = %id), name = "wt.remove")]
    pub fn remove_with(
        &self,
        id: &str,
        opts: RemoveOptions,
        mut on_step: impl FnMut(Step),
    ) -> Result<()> {
        let config = self.config()?;
        let state = self.state()?;
        let entry = state
            .worktrees
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow!("no tracked worktree with id '{}'", id))?;

        if entry.path.exists()
            && let Some(proj) = config.find_project(&entry.project)
        {
            on_step(Step::RemovingWorktree { path: &entry.path });
            git::worktree_remove(&proj.repo_path, &entry.path, opts.force)?;
        }

        if opts.delete_branch
            && let Some(proj) = config.find_project(&entry.project)
        {
            on_step(Step::DeletingBranch {
                branch: &entry.branch,
                force: opts.force,
            });
            git::branch_delete(&proj.repo_path, &entry.branch, opts.force)?;
        }

        on_step(Step::SavingMetadata);
        self.mutate(|state| {
            state.remove(id);
            Ok(())
        })
    }

    pub fn touch(&self, id: &str) -> Result<()> {
        self.mutate(|state| {
            state.touch(id);
            Ok(())
        })
    }
}

/// Undo a failed `create`: only what that call made. The target didn't exist
/// before it, and the branch is its own if it exists now but didn't then.
fn rollback_create(repo: &Path, target: &Path, branch: &str, branch_existed: bool) -> Result<()> {
    if target.exists() {
        git::worktree_remove(repo, target, true)?;
    }
    if !branch_existed && git::branch_exists(repo, branch)? {
        git::branch_delete(repo, branch, true)?;
    }
    Ok(())
}
