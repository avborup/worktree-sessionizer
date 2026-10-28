use std::cmp::Reverse;
use std::collections::HashSet;

use anyhow::Result;

use crate::layout;
use crate::state::{State, WorktreeEntry};
use crate::tmux;

mod picker;

#[derive(Debug, Clone)]
pub enum SwitchTarget {
    Worktree(WorktreeEntry),
    Session(String),
}

impl SwitchTarget {
    fn session_name(&self) -> &str {
        match self {
            Self::Worktree(e) => &e.session_name,
            Self::Session(name) => name,
        }
    }

    fn fuzzy_text(&self) -> String {
        match self {
            Self::Worktree(e) => format!("{} {} {}", e.project, e.branch, e.session_name),
            Self::Session(name) => name.clone(),
        }
    }

    fn worktree(&self) -> Option<&WorktreeEntry> {
        match self {
            Self::Worktree(e) => Some(e),
            Self::Session(_) => None,
        }
    }
}

#[tracing::instrument(level = "info", name = "switch.run")]
pub fn run(all_sessions: bool) -> Result<()> {
    let state = State::load()?;

    let mut live: Vec<_> = state
        .worktrees
        .iter()
        .filter(|e| e.path.exists())
        .cloned()
        .collect();
    live.sort_by_key(|e| Reverse(e.last_used_at));

    let mut targets: Vec<SwitchTarget> = live.into_iter().map(SwitchTarget::Worktree).collect();

    if all_sessions {
        let registered: HashSet<String> = targets
            .iter()
            .map(|t| t.session_name().to_string())
            .collect();
        let mut extra: Vec<_> = tmux::session_names_running()
            .into_iter()
            .filter(|name| !registered.contains(name))
            .collect();
        extra.sort_unstable();
        targets.extend(extra.into_iter().map(SwitchTarget::Session));
    }

    if targets.is_empty() {
        println!(
            "{}",
            if all_sessions {
                "no worktrees or tmux sessions"
            } else {
                "no tracked worktrees"
            }
        );
        return Ok(());
    }

    let selected = picker::pick(&targets)?;
    let Some(target) = selected else {
        return Ok(());
    };

    match target {
        SwitchTarget::Worktree(entry) => {
            if !tmux::session_exists(&entry.session_name) {
                let config = crate::config::Config::load()?;
                let proj = config.find_project(&entry.project).ok_or_else(|| {
                    anyhow::anyhow!("project '{}' not found in config", entry.project)
                })?;
                let pane_cmd = proj.pane_command.as_deref().unwrap_or("$SHELL");
                layout::create_session(&entry.session_name, &entry.path, pane_cmd)?;
            }

            State::mutate(|state| {
                state.touch(&entry.id);
                Ok(())
            })?;

            layout::attach_or_switch(&entry.session_name)?;
        }
        SwitchTarget::Session(name) => {
            layout::attach_or_switch(&name)?;
        }
    }

    Ok(())
}
