use std::io::Write;

use anyhow::{Result, bail};
use owo_colors::OwoColorize;

use crate::clone;
use crate::config::Config;
use crate::layout;
use crate::project;
use crate::state::{State, WorktreeEntry};
use crate::ui;
use crate::{git, tmux};

pub fn run(
    explicit_project: Option<&str>,
    branch: String,
    new_branch: Option<String>,
) -> Result<()> {
    let config = Config::load()?;
    let mut state = State::load()?;
    let proj = project::resolve(explicit_project, &config, &state)?;

    let (base, branch, is_new_branch) = match new_branch {
        Some(nb) => (Some(branch), nb, true),
        None => (None, branch, false),
    };

    let target = project::worktree_path(&config.worktrees_root, &proj.name, &branch);

    if target.exists() {
        bail!("worktree already exists at {}", target.display());
    }

    eprintln!(
        "\n  creating worktree for {} on {}\n",
        proj.name.magenta(),
        branch.white().bold(),
    );

    if is_new_branch {
        let base_ref = base.as_deref().unwrap();
        ui::step(&format!("branching {} from {}", branch.white(), base_ref.dimmed()));
        git::worktree_add_new_branch(&proj.repo_path, &target, &branch, base_ref)?;
        git::unset_upstream(&target)?;
    } else {
        ui::step(&format!("checking out {}", branch.white()));
        git::worktree_add(&proj.repo_path, &target, &branch)?;
    }
    ui::step_done(&format!("{}", target.display().dimmed()));

    for entry in &proj.cow_clone {
        let src = proj.repo_path.join(entry);
        let dst = target.join(entry);
        if src.exists() {
            ui::step(&format!("cow cloning {}", entry.cyan()));
            clone::cow_clone(&src, &dst)?;
        }
    }

    for entry in &proj.symlink {
        let src = proj.repo_path.join(entry);
        let dst = target.join(entry);
        if src.exists() {
            ui::step(&format!("symlinking {}", entry.cyan()));
            std::os::unix::fs::symlink(&src, &dst)?;
        }
    }

    eprintln!();
    let session_name = prompt_session_name()?;
    eprintln!();

    if tmux::session_exists(&session_name) {
        bail!("tmux session '{}' already exists", session_name);
    }

    let default_shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let pane_cmd = proj.pane_command.as_deref().unwrap_or(&default_shell);

    ui::step(&format!("creating session {}", session_name.green()));
    layout::create_session(&session_name, &target, pane_cmd)?;

    ui::step("saving metadata");
    let entry = WorktreeEntry::new(
        proj.name.clone(),
        branch.clone(),
        target,
        session_name.clone(),
        base.clone(),
    );
    state.add(entry);
    state.save()?;

    ui::step_done("done — attaching\n");
    layout::attach_or_switch(&session_name)?;

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
