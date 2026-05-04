use anyhow::Result;

use crate::layout;
use crate::state::State;
use crate::tmux;

mod picker;

#[tracing::instrument(level = "info", name = "switch.run")]
pub fn run() -> Result<()> {
    let mut state = State::load()?;

    let mut live: Vec<_> = state
        .worktrees
        .iter()
        .filter(|e| e.path.exists())
        .cloned()
        .collect();
    live.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));

    if live.is_empty() {
        println!("no tracked worktrees");
        return Ok(());
    }

    let selected = picker::pick(&live)?;
    let Some(entry) = selected else {
        return Ok(());
    };

    if !tmux::session_exists(&entry.session_name) {
        let config = crate::config::Config::load()?;
        let proj = config
            .find_project(&entry.project)
            .ok_or_else(|| anyhow::anyhow!("project '{}' not found in config", entry.project))?;
        let pane_cmd = proj.pane_command.as_deref().unwrap_or("$SHELL");
        layout::create_session(&entry.session_name, &entry.path, pane_cmd)?;
    }

    state.touch(&entry.id);
    state.save()?;

    layout::attach_or_switch(&entry.session_name)?;

    Ok(())
}
