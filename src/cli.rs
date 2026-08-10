use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wt", about = "Git worktree + tmux session manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Override project detection (use a registered project by name)
    #[arg(long, global = true)]
    pub project: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new worktree and tmux session
    New {
        /// Branch to check out (or base branch when two args given)
        branch: String,

        /// New branch to create from base (first arg becomes the base)
        new_branch: Option<String>,
    },

    /// Track an existing worktree at an arbitrary path
    Attach {
        /// Path to the existing worktree (defaults to cwd)
        path: Option<String>,
    },

    /// Fuzzy-pick a worktree and switch to its tmux session
    Switch {
        /// Include ad-hoc tmux sessions not tied to a tracked worktree
        #[arg(long)]
        all_sessions: bool,
    },

    /// List tracked worktrees
    List {
        /// Sort order
        #[arg(long, default_value = "last-used")]
        sort: SortOrder,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Show full details for each worktree
        #[arg(long, short)]
        verbose: bool,
    },

    /// Remove a tracked worktree
    Remove {
        /// Branch name or session name to identify the worktree
        name: String,

        /// Also delete the git branch
        #[arg(long)]
        branch: bool,

        /// Discard modified and untracked worktree files before removal
        #[arg(long)]
        force: bool,
    },

    /// Remove stale worktrees
    Prune {
        /// Stale threshold in days (default from config)
        #[arg(long)]
        days: Option<u32>,

        /// Accept defaults without prompting
        #[arg(long)]
        yes: bool,

        /// Also delete each worktree's git branch
        #[arg(long)]
        branch: bool,

        /// Discard modified and untracked worktree files before removal
        #[arg(long)]
        force: bool,
    },

    /// Show info about the current project
    Info {
        /// Print only the project name as a plain string (for shell interpolation)
        #[arg(long)]
        project_name: bool,
    },

    /// Manage registered projects
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Register a project
    Add {
        /// Project name (used in worktree paths)
        #[arg(long)]
        name: String,

        /// Path to the repository (defaults to cwd)
        #[arg(long)]
        repo_path: Option<String>,

        /// Command to run in each pane (defaults to $SHELL)
        #[arg(long)]
        pane_command: Option<String>,
    },

    /// List registered projects
    List,

    /// Unregister a project
    Remove {
        /// Project name
        #[arg(long)]
        name: String,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum SortOrder {
    LastUsed,
    Created,
    Branch,
    Project,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    #[test]
    fn remove_branch_and_force_parse_together() {
        let cli =
            Cli::try_parse_from(["wt", "remove", "feature/x", "--branch", "--force"]).unwrap();

        match cli.command {
            Command::Remove {
                name,
                branch,
                force,
            } => {
                assert_eq!(name, "feature/x");
                assert!(branch);
                assert!(force);
            }
            _ => panic!("expected remove command"),
        }
    }

    #[test]
    fn prune_branch_and_force_parse_together() {
        let cli =
            Cli::try_parse_from(["wt", "prune", "--yes", "--branch", "--force"]).unwrap();

        match cli.command {
            Command::Prune {
                yes, branch, force, ..
            } => {
                assert!(yes);
                assert!(branch);
                assert!(force);
            }
            _ => panic!("expected prune command"),
        }
    }
}
