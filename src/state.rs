use std::path::PathBuf;

use anyhow::{Context, Result};
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
