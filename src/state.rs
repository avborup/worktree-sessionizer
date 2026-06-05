use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use anyhow::{Context, Result};
use fs2::FileExt;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub worktrees: Vec<WorktreeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeEntry {
    pub id: String,
    pub project: String,
    pub branch: String,
    pub path: PathBuf,
    pub session_name: String,
    pub base_branch: Option<String>,
    pub created_at: Timestamp,
    pub last_used_at: Timestamp,
}

impl State {
    #[tracing::instrument(level = "info", name = "state.load")]
    pub fn load() -> Result<Self> {
        let path = state_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading state from {}", path.display()))?;
        toml::from_str(&contents).with_context(|| "parsing state.toml")
    }

    pub fn save(&self) -> Result<()> {
        let path = state_path();
        paths::atomic_write(&path, &toml::to_string_pretty(self)?)
    }

    /// Reload state under an exclusive lock, apply `f`, and persist.
    ///
    /// Use this for all writes so concurrent commands cannot clobber each other's
    /// updates while long-running work (e.g. cow clone) is in progress.
    pub fn mutate<F, T>(f: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let _lock = StateLock::acquire()?;
        let mut state = Self::load()?;
        let result = f(&mut state)?;
        state.save()?;
        Ok(result)
    }

    pub fn add(&mut self, entry: WorktreeEntry) {
        self.worktrees.push(entry);
    }

    pub fn remove(&mut self, id: &str) {
        self.worktrees.retain(|e| e.id != id);
    }

    pub fn touch(&mut self, id: &str) {
        if let Some(entry) = self.worktrees.iter_mut().find(|e| e.id == id) {
            entry.last_used_at = Timestamp::now();
        }
    }
}

impl WorktreeEntry {
    pub fn new(
        project: String,
        branch: String,
        path: PathBuf,
        session_name: String,
        base_branch: Option<String>,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            id: Ulid::new().to_string(),
            project,
            branch,
            path,
            session_name,
            base_branch,
            created_at: now,
            last_used_at: now,
        }
    }
}

fn state_path() -> PathBuf {
    paths::data_dir().join("state.toml")
}

fn lock_path() -> PathBuf {
    paths::data_dir().join("state.lock")
}

struct StateLock {
    _file: File,
}

impl StateLock {
    fn acquire() -> Result<Self> {
        let path = lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening lock file {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("acquiring lock on {}", path.display()))?;
        Ok(Self { _file: file })
    }
}
