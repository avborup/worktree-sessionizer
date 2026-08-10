use anyhow::{Result, bail};
use owo_colors::OwoColorize;

use crate::config::Config;
use crate::git;
use crate::state::State;
use crate::tmux;
use crate::ui;

#[tracing::instrument(level = "info", name = "remove.run")]
pub fn run(name: String, delete_branch: bool, force: bool) -> Result<()> {
    let config = Config::load()?;
    let state = State::load()?;

    let entry = state
        .worktrees
        .iter()
        .find(|e| e.branch == name || e.session_name == name)
        .cloned();

    let entry = match entry {
        Some(e) => e,
        None => bail!(
            "no tracked worktree matching '{}' (looked at branch and session name)",
            name
        ),
    };

    eprintln!(
        "\n  removing worktree {} on {}\n",
        entry.project.magenta(),
        entry.branch.white().bold(),
    );

    if tmux::session_exists(&entry.session_name) {
        ui::step(&format!("killing session {}", entry.session_name.yellow()));
        tmux::kill_session(&entry.session_name)?;
    }

    if entry.path.exists()
        && let Some(proj) = config.find_project(&entry.project)
    {
        ui::step(&format!("removing worktree at {}", entry.path.display().dimmed()));
        git::worktree_remove(&proj.repo_path, &entry.path, force)?;
    }

    if delete_branch
        && let Some(proj) = config.find_project(&entry.project)
    {
        let label = if force { "force-deleting" } else { "deleting" };
        ui::step(&format!("{} branch {}", label, entry.branch.yellow()));
        git::branch_delete(&proj.repo_path, &entry.branch, force)?;
    }

    ui::step("saving metadata");
    let entry_id = entry.id.clone();
    State::mutate(|state| {
        state.remove(&entry_id);
        Ok(())
    })?;

    ui::step_done(&format!(
        "removed {} · {}\n",
        entry.project.magenta(),
        entry.branch.dimmed()
    ));

    Ok(())
}
