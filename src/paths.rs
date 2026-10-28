use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `$WT_HOME` when set, so tests and other tools can point wt at their own
/// config and state, else the platform data dir.
pub fn data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("WT_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home);
    }
    directories::ProjectDirs::from("", "", "wt")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".wt"))
}

pub fn config_path(home: &Path) -> PathBuf {
    home.join("config.toml")
}

pub fn state_path(home: &Path) -> PathBuf {
    home.join("state.toml")
}

pub fn lock_path(home: &Path) -> PathBuf {
    home.join("state.lock")
}

/// Plain-text cache: one running tmux session name per line (`tmux list-sessions -F '#{session_name}'`).
pub fn tmux_sessions_cache_path() -> PathBuf {
    data_dir().join("tmux_sessions.cache")
}

pub fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("creating temp file in {}", dir.display()))?;
    std::io::Write::write_all(&mut tmp, contents.as_bytes())?;
    tmp.persist(path)
        .with_context(|| format!("persisting to {}", path.display()))?;
    Ok(())
}
