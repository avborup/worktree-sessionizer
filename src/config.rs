use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub worktrees_root: PathBuf,
    pub prune_after_days: u32,
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub repo_path: PathBuf,
    pub pane_command: Option<String>,
    #[serde(default)]
    pub cow_clone: Vec<String>,
    #[serde(default)]
    pub symlink: Vec<String>,
    /// Named commands run by `wt gc <name>` in the repo and every git worktree
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gc: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs_home().unwrap_or_else(|| PathBuf::from("."));
        Self {
            worktrees_root: home.join("coding/.worktrees"),
            prune_after_days: 30,
            projects: Vec::new(),
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_in(&paths::data_dir())
    }

    /// The config in `home` (a wt data dir), or the default when it has none.
    #[tracing::instrument(level = "info", name = "config.load")]
    pub fn load_in(home: &Path) -> Result<Self> {
        let path = paths::config_path(home);
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        toml::from_str(&contents).with_context(|| "parsing config.toml")
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_path(&paths::data_dir());
        paths::atomic_write(&path, &toml::to_string_pretty(self)?)
    }

    pub fn find_project(&self, name: &str) -> Option<&ProjectConfig> {
        self.projects.iter().find(|p| p.name == name)
    }

    pub fn find_project_by_path(&self, dir: &Path) -> Option<&ProjectConfig> {
        self.projects.iter().find(|p| dir.starts_with(&p.repo_path))
    }
}
