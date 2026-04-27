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
mod ui;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::New { branch, new_branch } => {
            commands::new::run(cli.project.as_deref(), branch, new_branch)
        }
        cli::Command::Attach { path } => {
            commands::attach::run(cli.project.as_deref(), path)
        }
        cli::Command::Switch => commands::switch::run(),
        cli::Command::Remove {
            name,
            branch,
            force_branch,
        } => commands::remove::run(name, branch, force_branch),
        cli::Command::List {
            sort,
            json,
            verbose,
        } => commands::list::run(sort, json, verbose),
        cli::Command::Prune { days, yes } => commands::prune::run(days, yes),
        cli::Command::Project { action } => commands::project::run(action),
    }
}
