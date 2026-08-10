mod cli;
mod clone;
mod commands;
mod config;
mod git;
mod layout;
mod paths;
mod project;
mod state;
mod tmux;
mod trace;
mod ui;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    trace::init();

    let _root = tracing::info_span!("wt").entered();

    let cli = tracing::info_span!("cli.parse").in_scope(cli::Cli::parse);

    match cli.command {
        cli::Command::New { branch, new_branch } => {
            commands::new::run(cli.project.as_deref(), branch, new_branch)
        }
        cli::Command::Attach { path } => {
            commands::attach::run(cli.project.as_deref(), path)
        }
        cli::Command::Switch { all_sessions } => commands::switch::run(all_sessions),
        cli::Command::Remove {
            name,
            branch,
            force,
        } => commands::remove::run(name, branch, force),
        cli::Command::List {
            sort,
            json,
            verbose,
        } => commands::list::run(sort, json, verbose),
        cli::Command::Prune {
            days,
            yes,
            branch,
            force,
        } => commands::prune::run(days, yes, branch, force),
        cli::Command::Info { project_name } => {
            commands::info::run(cli.project.as_deref(), project_name)
        }
        cli::Command::Project { action } => commands::project::run(action),
    }
}
