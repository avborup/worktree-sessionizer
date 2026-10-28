use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;

use anyhow::{Context, Result, bail};
use wt::paths;
use wt::state::WorktreeEntry;

/// Initial alive flags for the switch picker, optionally paired with a channel that delivers a
/// fresher vector after a background `tmux list-sessions`.
pub struct PickerPresence {
    pub alive: Vec<bool>,
    pub updates: Option<mpsc::Receiver<Vec<bool>>>,
}

pub fn prepare_picker_presence(entries: &[WorktreeEntry]) -> PickerPresence {
    match load_cached_session_names() {
        Some(snapshot) => PickerPresence {
            alive: alive_flags(entries, &snapshot),
            updates: Some(spawn_alive_refresh(entries)),
        },
        None => {
            let running = session_names_running();
            save_session_names_cache(&running);
            PickerPresence {
                alive: alive_flags(entries, &running),
                updates: None,
            }
        }
    }
}

/// One `tmux list-sessions` plus membership checks — used by `wt list` and the picker cold path.
pub fn sessions_alive(entries: &[WorktreeEntry]) -> Vec<bool> {
    alive_flags(entries, &session_names_running())
}

pub fn session_names_running() -> HashSet<String> {
    list_sessions_stdout()
        .map(|raw| parse_session_names(&raw))
        .unwrap_or_default()
}

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

fn list_sessions_stdout() -> Option<Vec<u8>> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn parse_session_names(stdout: &[u8]) -> HashSet<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}

fn alive_flags(entries: &[WorktreeEntry], running: &HashSet<String>) -> Vec<bool> {
    entries
        .iter()
        .map(|e| running.contains(&e.session_name))
        .collect()
}

fn spawn_alive_refresh(entries: &[WorktreeEntry]) -> mpsc::Receiver<Vec<bool>> {
    let names: Vec<String> = entries.iter().map(|e| e.session_name.clone()).collect();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let running = session_names_running();
        save_session_names_cache(&running);
        let alive: Vec<bool> = names.iter().map(|n| running.contains(n)).collect();
        let _ = tx.send(alive);
    });

    rx
}

fn load_cached_session_names() -> Option<HashSet<String>> {
    let raw = std::fs::read_to_string(paths::tmux_sessions_cache_path()).ok()?;
    Some(
        raw.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect(),
    )
}

fn save_session_names_cache(names: &HashSet<String>) {
    let mut lines: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    lines.sort_unstable();
    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    let _ = paths::atomic_write(&paths::tmux_sessions_cache_path(), &body);
}
