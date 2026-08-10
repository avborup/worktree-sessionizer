use std::io::Write;

use anyhow::Result;
use jiff::Timestamp;
use owo_colors::OwoColorize;

use crate::config::Config;
use crate::git;
use crate::state::State;
use crate::ui;

#[tracing::instrument(level = "info", name = "prune.run")]
pub fn run(
    days_override: Option<u32>,
    auto_yes: bool,
    delete_branch: bool,
    force: bool,
) -> Result<()> {
    let config = Config::load()?;
    let state = State::load()?;

    let threshold_days = days_override.unwrap_or(config.prune_after_days);
    let now = Timestamp::now();

    let stale_ids: Vec<_> = state
        .worktrees
        .iter()
        .filter(|e| {
            ui::Age::from_timestamps(now, e.last_used_at).days() >= threshold_days as i64
        })
        .map(|e| e.id.clone())
        .collect();

    if stale_ids.is_empty() {
        eprintln!(
            "  {} no worktrees older than {} days\n",
            "✓".green(),
            threshold_days
        );
        return Ok(());
    }

    eprintln!(
        "\n  found {} stale worktree(s) (>{} days)\n",
        stale_ids.len().to_string().yellow(),
        threshold_days
    );

    let mut to_remove = Vec::new();

    for id in &stale_ids {
        let entry = state.worktrees.iter().find(|e| &e.id == id).unwrap();
        let age_days = ui::Age::from_timestamps(now, entry.last_used_at).days();

        let proj = config.find_project(&entry.project);
        let base = entry.base_branch.as_deref().unwrap_or("main");
        let merged = proj
            .map(|p| git::is_branch_merged(&p.repo_path, &entry.branch, base).unwrap_or(false))
            .unwrap_or(false);

        let merged_label = if merged {
            "yes".green().to_string()
        } else {
            "no".red().to_string()
        };

        eprintln!(
            "  {} {} · {} · last used {} days ago",
            "·".dimmed(),
            entry.project.magenta(),
            entry.branch.white(),
            age_days.to_string().yellow()
        );
        eprintln!(
            "    {} {}",
            "path".dimmed(),
            entry.path.display().dimmed()
        );
        eprintln!(
            "    {} {} (merged into {}: {})",
            "branch".dimmed(),
            entry.branch.dimmed(),
            base.dimmed(),
            merged_label
        );

        let default_action = Action::RemoveOnly;

        let action = if auto_yes {
            default_action
        } else {
            prompt_action(default_action)?
        };

        match action {
            Action::Skip => {
                eprintln!("    {}\n", "skipped".dimmed());
            }
            _ => {
                to_remove.push((id.clone(), entry.branch.clone(), entry.path.clone()));
                eprintln!();
            }
        }
    }

    let mut removed_ids = Vec::new();

    for (id, branch, path) in &to_remove {
        let proj_entry = state.worktrees.iter().find(|e| &e.id == id);
        let repo_path = proj_entry.and_then(|e| {
            config
                .find_project(&e.project)
                .map(|p| p.repo_path.clone())
        });

        let worktree_removed = if path.exists()
            && let Some(repo) = &repo_path
        {
            ui::step(&format!("removing worktree {}", path.display().dimmed()));
            match git::worktree_remove(repo, path, force) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("    {} {}", "!".red(), e);
                    false
                }
            }
        } else {
            true
        };

        if !worktree_removed {
            continue;
        }

        if delete_branch
            && let Some(repo) = &repo_path
        {
            let label = if force { "force-deleting" } else { "deleting" };
            ui::step(&format!("{} branch {}", label, branch.yellow()));
            if let Err(e) = git::branch_delete(repo, branch, force) {
                eprintln!("    {} {}", "!".red(), e);
            }
        }

        removed_ids.push(id.clone());
    }

    let count = removed_ids.len();
    State::mutate(|state| {
        for id in removed_ids {
            state.remove(&id);
        }
        Ok(())
    })?;

    if count > 0 {
        ui::step_done(&format!("pruned {} worktree(s)\n", count));
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum Action {
    RemoveOnly,
    Skip,
}

fn prompt_action(default: Action) -> Result<Action> {
    let default_char = match default {
        Action::RemoveOnly => 'r',
        Action::Skip => 's',
    };
    eprint!(
        "    {} [{}]emove worktree / [{}]kip [{}]: ",
        "?".cyan(),
        "r".cyan(),
        "s".cyan(),
        default_char.to_string().white().bold(),
    );
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        return Ok(default);
    }

    match input {
        "r" => Ok(Action::RemoveOnly),
        "s" => Ok(Action::Skip),
        _ => {
            eprintln!("    {}", "invalid choice, skipping".dimmed());
            Ok(Action::Skip)
        }
    }
}
