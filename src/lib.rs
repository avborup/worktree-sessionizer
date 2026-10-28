//! wt's worktree bookkeeping without tmux: the config, the state file and the
//! git steps behind `wt new` and `wt remove`. The `wt` binary (the `cli`
//! feature) adds the tmux sessions, the picker and the terminal output.

mod api;
pub mod clone;
pub mod config;
pub mod git;
pub mod paths;
pub mod project;
pub mod state;

pub use api::{CreateWorktree, RemoveOptions, Step, Wt};
pub use config::{Config, ProjectConfig};
pub use state::{State, WorktreeEntry};
