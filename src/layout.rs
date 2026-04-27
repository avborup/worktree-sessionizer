use std::path::Path;

use anyhow::Result;

use crate::tmux;

pub fn create_session(name: &str, worktree: &Path, pane_command: &str) -> Result<()> {
    let left_pane = tmux::new_session(name, worktree)?;

    tmux::send_keys(&left_pane, pane_command)?;

    let right_pane = tmux::split_window(&left_pane, worktree)?;
    tmux::send_keys(&right_pane, pane_command)?;

    tmux::new_window(name, "server", worktree)?;
    let server_target = format!("{name}:server");
    tmux::send_keys(&server_target, pane_command)?;

    tmux::select_window(name, 0)?;
    tmux::select_pane(&left_pane)?;

    Ok(())
}

pub fn attach_or_switch(name: &str) -> Result<()> {
    if tmux::is_inside_tmux() {
        tmux::switch_client(name)
    } else {
        tmux::attach(name)
    }
}
