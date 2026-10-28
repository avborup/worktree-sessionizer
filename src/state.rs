use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

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
    pub fn load() -> Result<Self> {
        Self::load_in(&paths::data_dir())
    }

    /// The state in `home` (a wt data dir), read without the lock.
    #[tracing::instrument(level = "info", name = "state.load")]
    pub fn load_in(home: &Path) -> Result<Self> {
        let path = paths::state_path(home);
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading state from {}", path.display()))?;
        toml::from_str(&contents).with_context(|| "parsing state.toml")
    }

    fn save_in(&self, home: &Path) -> Result<()> {
        let path = paths::state_path(home);
        paths::atomic_write(&path, &toml::to_string_pretty(self)?)
    }

    pub fn mutate<F, T>(f: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        Self::mutate_in(&paths::data_dir(), f)
    }

    /// Reload state under an exclusive lock, apply `f`, and persist.
    ///
    /// Use this for all writes so concurrent commands cannot clobber each other's
    /// updates while long-running work (e.g. cow clone) is in progress.
    pub fn mutate_in<F, T>(home: &Path, f: F) -> Result<T>
    where
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let _lock = StateLock::acquire(home)?;
        let mut state = Self::load_in(home)?;
        let result = f(&mut state)?;
        state.save_in(home)?;
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

struct StateLock {
    _file: File,
}

impl StateLock {
    fn acquire(home: &Path) -> Result<Self> {
        let path = paths::lock_path(home);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening lock file {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("acquiring lock on {}", path.display()))?;
        Ok(Self { _file: file })
    }
}
