use anyhow::Result;
use jiff::Timestamp;
use owo_colors::OwoColorize;

use crate::cli::SortOrder;
use crate::state::{State, WorktreeEntry};
use crate::tmux;
use crate::ui::{icons, Age};

#[tracing::instrument(level = "info", name = "list.run")]
pub fn run(sort: SortOrder, json: bool, verbose: bool) -> Result<()> {
    let state = State::load()?;

    if state.worktrees.is_empty() {
        println!("no tracked worktrees");
        return Ok(());
    }

    let mut entries = state.worktrees.clone();

    match sort {
        SortOrder::LastUsed => entries.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at)),
        SortOrder::Created => entries.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        SortOrder::Branch => entries.sort_by(|a, b| a.branch.cmp(&b.branch)),
        SortOrder::Project => entries.sort_by(|a, b| a.project.cmp(&b.project)),
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    let now = Timestamp::now();

    if verbose {
        print_verbose(&entries, now);
    } else {
        print_compact(&entries, now);
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

fn print_compact(entries: &[WorktreeEntry], now: Timestamp) {
    let term_width = terminal_width();
    let proj_w = 10;
    let branch_w = (term_width / 3).clamp(16, 36);
    let session_w: usize = 16;
    let age_w = 10;

    let branch_pad = branch_w.saturating_sub(1);
    println!(
        "  {:<proj_w$} {:<age_w$} {:<session_w$}  {:<branch_pad$}",
        "PROJECT".dimmed(),
        "AGE".dimmed(),
        "SESSION".dimmed(),
        "BRANCH".dimmed(),
    );

    for e in entries {
        let alive = tmux::session_exists(&e.session_name);
        let age = Age::from_timestamps(now, e.last_used_at);
        let bracket = age.bracket();

        let status = if alive {
            "  ".to_string()
        } else {
            icons::INACTIVE.dimmed().to_string()
        };

        let branch_trunc = truncate(&e.branch, branch_w);
        let session_trunc = truncate(&e.session_name, session_w);
        let age_label = bracket.paint(&format!("{:<age_w$}", age.label()));

        let branch_pad = branch_w.saturating_sub(1);
        println!(
            "{}{:<proj_w$} {} {:<session_w$} {}{:<branch_pad$}",
            status,
            e.project.magenta(),
            age_label,
            session_trunc.white(),
            icons::BRANCH.dimmed(),
            branch_trunc.dimmed(),
        );
    }
}

fn print_verbose(entries: &[WorktreeEntry], now: Timestamp) {
    for (i, e) in entries.iter().enumerate() {
        let alive = tmux::session_exists(&e.session_name);
        let age = Age::from_timestamps(now, e.last_used_at);
        let bracket = age.bracket();

        let status = if alive {
            " ".to_string()
        } else {
            icons::INACTIVE.dimmed().to_string()
        };

        if i > 0 {
            println!();
        }

        println!(
            "{} {} {}",
            status,
            e.project.magenta(),
            bracket.paint(&age.label()),
        );
        println!(
            "  {} {}",
            "branch".dimmed(),
            e.branch.white(),
        );
        println!(
            "  {} {}",
            "session".dimmed(),
            e.session_name,
        );
        println!(
            "  {} {}",
            "path".dimmed(),
            e.path.display().dimmed(),
        );
        if let Some(base) = &e.base_branch {
            println!(
                "  {} {}",
                "base".dimmed(),
                base.dimmed(),
            );
        }
    }
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}
