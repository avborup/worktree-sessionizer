use std::io::Write;

use anyhow::{Result, bail};
use owo_colors::OwoColorize;
use wt::{CreateWorktree, Wt, project};

use crate::layout;
use crate::tmux;
use crate::ui;

#[tracing::instrument(level = "info", name = "new.run")]
pub fn run(
    explicit_project: Option<&str>,
    branch: String,
    new_branch: Option<String>,
) -> Result<()> {
    let wt = Wt::from_env();
    let config = wt.config()?;
    let state = wt.state()?;
    let proj = project::resolve(explicit_project, &config, &state)?;

    let (base, branch) = match new_branch {
        Some(nb) => (Some(branch), nb),
        None => (None, branch),
    };

    // `create` checks this too; checking here refuses before the prompt.
    let target = project::worktree_path(&config.worktrees_root, &proj.name, &branch);
    if target.exists() {
        bail!("worktree already exists at {}", target.display());
    }

    eprintln!(
        "\n  creating worktree for {} on {}\n",
        proj.name.magenta(),
        branch.white().bold(),
    );

    let session_name = prompt_session_name()?;
    eprintln!();

    if tmux::session_exists(&session_name) {
        bail!("tmux session '{}' already exists", session_name);
    }

    let entry = wt.create_with(
        &CreateWorktree {
            project: proj.name.clone(),
            branch,
            base,
            session_name,
        },
        ui::progress,
    )?;

    let default_shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let pane_cmd = proj.pane_command.as_deref().unwrap_or(&default_shell);

    ui::step(&format!("creating session {}", entry.session_name.green()));
    layout::create_session(&entry.session_name, &entry.path, pane_cmd)?;

    ui::step_done("done — attaching\n");
    layout::attach_or_switch(&entry.session_name)?;

    Ok(())
}

fn prompt_session_name() -> Result<String> {
    eprint!("  {} session name: ", "?".cyan());
    std::io::stderr().flush()?;
    let mut name = String::new();
    std::io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        bail!("session name required");
    }
    Ok(name)
}
