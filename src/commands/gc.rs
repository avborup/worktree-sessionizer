use std::process::Command;

use anyhow::{Context, Result, bail};
use owo_colors::OwoColorize;

use crate::config::Config;
use crate::git;
use crate::project;
use crate::state::State;
use crate::ui;

#[tracing::instrument(level = "info", name = "gc.run")]
pub fn run(explicit_project: Option<&str>, name: String, dry_run: bool) -> Result<()> {
    let config = Config::load()?;
    let state = State::load()?;

    let proj = project::resolve(explicit_project, &config, &state)?;

    let Some(command) = proj.gc.get(&name) else {
        let available: Vec<&str> = proj.gc.keys().map(String::as_str).collect();
        bail!(
            "no gc command '{}' for project '{}' (available: {})",
            name,
            proj.name,
            if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            }
        );
    };

    let dirs: Vec<_> = git::worktree_list(&proj.repo_path)?
        .into_iter()
        .filter(|dir| dir.exists())
        .collect();

    eprintln!(
        "\n  gc {} on {} ({} dirs)",
        name.white().bold(),
        proj.name.magenta(),
        dirs.len()
    );
    eprintln!("    {} {}\n", "command".dimmed(), command.dimmed());

    if dry_run {
        for dir in &dirs {
            eprintln!("  {} {}", "·".dimmed(), dir.display());
        }
        eprintln!();
        return Ok(());
    }

    let before = fs2::available_space(&proj.repo_path)?;
    let mut failed = Vec::new();

    for dir in &dirs {
        ui::step(&dir.display().to_string());

        let status = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(dir)
            .status()
            .with_context(|| format!("running gc in {}", dir.display()))?;

        if !status.success() {
            eprintln!("  {} exited with {}", "✗".red(), status);
            failed.push(dir);
        }
    }

    let after = fs2::available_space(&proj.repo_path)?;
    ui::step_done(&format!(
        "freed {}\n",
        format_bytes(after.saturating_sub(before)).green()
    ));

    if !failed.is_empty() {
        bail!("gc failed in {} of {} dirs", failed.len(), dirs.len());
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes / GIB)
    } else {
        format!("{:.0} MiB", bytes / MIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_picks_unit() {
        assert_eq!(format_bytes(5 * 1024 * 1024), "5 MiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024 / 2), "1.5 GiB");
    }
}
