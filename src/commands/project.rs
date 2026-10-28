use anyhow::{Result, bail};
use owo_colors::OwoColorize;
use wt::config::{Config, ProjectConfig};

use crate::cli::ProjectAction;
use crate::ui;

#[tracing::instrument(level = "info", name = "project.run", skip(action))]
pub fn run(action: ProjectAction) -> Result<()> {
    let mut config = Config::load()?;

    match action {
        ProjectAction::Add {
            name,
            repo_path,
            pane_command,
        } => {
            if config.find_project(&name).is_some() {
                bail!("project '{}' already exists", name);
            }
            let repo_path = match repo_path {
                Some(p) => std::path::PathBuf::from(p),
                None => std::env::current_dir()?,
            };
            let repo_path = repo_path.canonicalize()?;

            eprintln!("\n  registering project {}\n", name.magenta());

            ui::step(&format!("repo at {}", repo_path.display().dimmed()));

            if let Some(cmd) = &pane_command {
                ui::step(&format!("pane command {}", cmd.cyan()));
            }

            config.projects.push(ProjectConfig {
                name: name.clone(),
                repo_path,
                pane_command,
                cow_clone: Vec::new(),
                symlink: Vec::new(),
                gc: Default::default(),
            });
            config.save()?;

            ui::step_done(&format!("added {}\n", name.magenta()));
        }
        ProjectAction::List => {
            if config.projects.is_empty() {
                eprintln!("  no projects registered — run {}", "wt project add".cyan());
                return Ok(());
            }
            eprintln!();
            for p in &config.projects {
                let cmd = p.pane_command.as_deref().unwrap_or("$SHELL");
                eprintln!("  {} {}", "·".dimmed(), p.name.magenta());
                eprintln!("    {} {}", "repo".dimmed(), p.repo_path.display().dimmed());
                eprintln!("    {} {}", "shell".dimmed(), cmd.dimmed());
                if !p.cow_clone.is_empty() {
                    eprintln!(
                        "    {} {}",
                        "clone".dimmed(),
                        p.cow_clone.join(", ").dimmed()
                    );
                }
                if !p.symlink.is_empty() {
                    eprintln!("    {} {}", "link".dimmed(), p.symlink.join(", ").dimmed());
                }
                eprintln!();
            }
        }
        ProjectAction::Remove { name } => {
            let before = config.projects.len();
            config.projects.retain(|p| p.name != name);
            if config.projects.len() == before {
                bail!("no project named '{}'", name);
            }
            config.save()?;
            ui::step_done(&format!("removed project {}", name.magenta()));
        }
    }
    Ok(())
}
