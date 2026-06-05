use std::io::Write;
use std::path::PathBuf;

use anyhow::{Result, bail};
use owo_colors::OwoColorize;

use crate::config::Config;
use crate::git;
use crate::project;
use crate::state::{State, WorktreeEntry};
use crate::ui;

#[tracing::instrument(level = "info", name = "attach.run")]
pub fn run(explicit_project: Option<&str>, path: Option<String>) -> Result<()> {
    let config = Config::load()?;
    let state = State::load()?;

    let worktree_path = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let worktree_path = worktree_path.canonicalize()?;

    if !worktree_path.exists() {
        bail!("path does not exist: {}", worktree_path.display());
    }

    if state.worktrees.iter().any(|e| e.path == worktree_path) {
        bail!("worktree already tracked: {}", worktree_path.display());
    }

    let branch = git::current_branch(&worktree_path)?;
    let proj = project::resolve(explicit_project, &config, &state)?;

    eprintln!(
        "\n  attaching worktree to {}\n",
        proj.name.magenta(),
    );

    ui::step(&format!("detected branch {}", branch.white()));
    ui::step(&format!("path {}", worktree_path.display().dimmed()));

    eprintln!();
    let session_name = prompt_session_name(&branch)?;
    eprintln!();

    let entry = WorktreeEntry::new(
        proj.name.clone(),
        branch.clone(),
        worktree_path,
        session_name,
        None,
    );
    State::mutate(|state| {
        if state.worktrees.iter().any(|e| e.path == entry.path) {
            bail!("worktree already tracked: {}", entry.path.display());
        }
        state.add(entry);
        Ok(())
    })?;

    ui::step_done(&format!(
        "attached {} · {}\n",
        proj.name.magenta(),
        branch.dimmed()
    ));

    Ok(())
}

fn prompt_session_name(branch: &str) -> Result<String> {
    let default = project::sanitize_branch(branch);
    eprint!("  {} session name [{}]: ", "?".cyan(), default.dimmed());
    std::io::stderr().flush()?;
    let mut name = String::new();
    std::io::stdin().read_line(&mut name)?;
    let name = name.trim();
    if name.is_empty() {
        Ok(default)
    } else {
        Ok(name.to_string())
    }
}
