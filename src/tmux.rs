use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok_and(|v| !v.is_empty())
}

pub fn new_session(name: &str, cwd: &Path) -> Result<String> {
    let out = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            &cwd.to_string_lossy(),
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .output()
        .context("tmux new-session")?;
    if !out.status.success() {
        bail!(
            "tmux new-session failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn split_window(target: &str, cwd: &Path) -> Result<String> {
    let out = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-t",
            target,
            "-c",
            &cwd.to_string_lossy(),
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .output()
        .context("tmux split-window")?;
    if !out.status.success() {
        bail!(
            "tmux split-window failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn new_window(session: &str, name: &str, cwd: &Path) -> Result<()> {
    let out = Command::new("tmux")
        .args([
            "new-window",
            "-t",
            session,
            "-n",
            name,
            "-c",
            &cwd.to_string_lossy(),
        ])
        .output()
        .context("tmux new-window")?;
    if !out.status.success() {
        bail!(
            "tmux new-window failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn send_keys(target: &str, keys: &str) -> Result<()> {
    Command::new("tmux")
        .args(["send-keys", "-t", target, keys, "Enter"])
        .output()
        .context("tmux send-keys")?;
    Ok(())
}

pub fn select_window(session: &str, index: u32) -> Result<()> {
    Command::new("tmux")
        .args(["select-window", "-t", &format!("{session}:{index}")])
        .output()
        .context("tmux select-window")?;
    Ok(())
}

pub fn select_pane(pane_id: &str) -> Result<()> {
    Command::new("tmux")
        .args(["select-pane", "-t", pane_id])
        .output()
        .context("tmux select-pane")?;
    Ok(())
}

pub fn attach(name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["attach", "-t", name])
        .status()
        .context("tmux attach")?;
    if !status.success() {
        bail!("tmux attach failed");
    }
    Ok(())
}

pub fn switch_client(name: &str) -> Result<()> {
    let out = Command::new("tmux")
        .args(["switch-client", "-t", name])
        .output()
        .context("tmux switch-client")?;
    if !out.status.success() {
        bail!(
            "tmux switch-client failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

pub fn kill_session(name: &str) -> Result<()> {
    let out = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()
        .context("tmux kill-session")?;
    if !out.status.success() {
        bail!(
            "tmux kill-session failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

