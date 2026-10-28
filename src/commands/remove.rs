use anyhow::{Result, bail};
use owo_colors::OwoColorize;
use wt::{RemoveOptions, Wt};

use crate::tmux;
use crate::ui;

#[tracing::instrument(level = "info", name = "remove.run")]
pub fn run(name: String, delete_branch: bool, force: bool) -> Result<()> {
    let wt = Wt::from_env();
    let state = wt.state()?;

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

    wt.remove_with(
        &entry.id,
        RemoveOptions {
            delete_branch,
            force,
        },
        ui::progress,
    )?;

    ui::step_done(&format!(
        "removed {} · {}\n",
        entry.project.magenta(),
        entry.branch.dimmed()
    ));

    Ok(())
}
