use std::path::Path;

use anyhow::Result;
use jiff::Timestamp;
use owo_colors::OwoColorize;
use wt::config::{Config, ProjectConfig};
use wt::project;
use wt::state::{State, WorktreeEntry};

use crate::ui::Age;

#[tracing::instrument(level = "info", name = "info.run")]
pub fn run(explicit_project: Option<&str>, project_name: bool) -> Result<()> {
    let config = Config::load()?;
    let state = State::load()?;

    let proj = project::resolve(explicit_project, &config, &state)?;

    if project_name {
        println!("{}", proj.name);
        return Ok(());
    }

    let cwd = std::env::current_dir()?;
    let worktree = current_worktree(&state, &cwd);

    print!("{}", render(&proj, worktree, Timestamp::now()));
    Ok(())
}

fn current_worktree<'a>(state: &'a State, cwd: &Path) -> Option<&'a WorktreeEntry> {
    state.worktrees.iter().find(|e| cwd.starts_with(&e.path))
}

fn render(project: &ProjectConfig, worktree: Option<&WorktreeEntry>, now: Timestamp) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    let cmd = project.pane_command.as_deref().unwrap_or("$SHELL");

    let _ = writeln!(out);
    let _ = writeln!(out, "  {} {}", "·".dimmed(), project.name.magenta());
    let _ = writeln!(
        out,
        "    {} {}",
        "repo".dimmed(),
        project.repo_path.display().dimmed()
    );
    let _ = writeln!(out, "    {} {}", "shell".dimmed(), cmd.dimmed());
    if !project.cow_clone.is_empty() {
        let _ = writeln!(
            out,
            "    {} {}",
            "clone".dimmed(),
            project.cow_clone.join(", ").dimmed()
        );
    }
    if !project.symlink.is_empty() {
        let _ = writeln!(
            out,
            "    {} {}",
            "link".dimmed(),
            project.symlink.join(", ").dimmed()
        );
    }
    if !project.gc.is_empty() {
        let names: Vec<&str> = project.gc.keys().map(String::as_str).collect();
        let _ = writeln!(out, "    {} {}", "gc".dimmed(), names.join(", ").dimmed());
    }

    if let Some(e) = worktree {
        let age = Age::from_timestamps(now, e.last_used_at);
        let _ = writeln!(out);
        let _ = writeln!(out, "    {} {}", "branch".dimmed(), e.branch.white());
        let _ = writeln!(out, "    {} {}", "session".dimmed(), e.session_name);
        let _ = writeln!(
            out,
            "    {} {}",
            "age".dimmed(),
            age.bracket().paint(&age.label())
        );
        let _ = writeln!(out, "    {} {}", "path".dimmed(), e.path.display().dimmed());
        if let Some(base) = &e.base_branch {
            let _ = writeln!(out, "    {} {}", "base".dimmed(), base.dimmed());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_project() -> ProjectConfig {
        ProjectConfig {
            name: "myapp".to_string(),
            repo_path: PathBuf::from("/coding/myapp"),
            pane_command: Some("fish".to_string()),
            cow_clone: Vec::new(),
            symlink: Vec::new(),
            gc: Default::default(),
        }
    }

    fn sample_entry(path: &str) -> WorktreeEntry {
        WorktreeEntry::new(
            "myapp".to_string(),
            "feat/x".to_string(),
            PathBuf::from(path),
            "myapp-feat-x".to_string(),
            Some("main".to_string()),
        )
    }

    #[test]
    fn current_worktree_matches_exact_path() {
        let mut state = State::default();
        state.worktrees.push(sample_entry("/wt/myapp/feat-x"));
        let found = current_worktree(&state, Path::new("/wt/myapp/feat-x"));
        assert_eq!(found.map(|e| e.branch.as_str()), Some("feat/x"));
    }

    #[test]
    fn current_worktree_matches_nested_cwd() {
        let mut state = State::default();
        state.worktrees.push(sample_entry("/wt/myapp/feat-x"));
        let found = current_worktree(&state, Path::new("/wt/myapp/feat-x/src/sub"));
        assert!(found.is_some());
    }

    #[test]
    fn current_worktree_none_when_outside() {
        let mut state = State::default();
        state.worktrees.push(sample_entry("/wt/myapp/feat-x"));
        let found = current_worktree(&state, Path::new("/somewhere/else"));
        assert!(found.is_none());
    }

    #[test]
    fn render_includes_project_fields() {
        let p = sample_project();
        let out = render(&p, None, Timestamp::now());
        assert!(out.contains("myapp"));
        assert!(out.contains("/coding/myapp"));
        assert!(out.contains("fish"));
    }

    #[test]
    fn render_defaults_shell_when_no_pane_command() {
        let mut p = sample_project();
        p.pane_command = None;
        let out = render(&p, None, Timestamp::now());
        assert!(out.contains("$SHELL"));
    }

    #[test]
    fn render_clone_and_link_only_when_present() {
        let mut p = sample_project();
        let out = render(&p, None, Timestamp::now());
        assert!(!out.contains("clone"));
        assert!(!out.contains("link"));

        p.cow_clone = vec!["target".to_string()];
        p.symlink = vec![".env".to_string()];
        let out = render(&p, None, Timestamp::now());
        assert!(out.contains("clone"));
        assert!(out.contains("target"));
        assert!(out.contains("link"));
        assert!(out.contains(".env"));
    }

    #[test]
    fn render_includes_worktree_section_when_present() {
        let p = sample_project();
        let e = sample_entry("/wt/myapp/feat-x");
        let out = render(&p, Some(&e), Timestamp::now());
        assert!(out.contains("feat/x"));
        assert!(out.contains("myapp-feat-x"));
        assert!(out.contains("session"));
    }

    #[test]
    fn render_omits_worktree_section_when_absent() {
        let p = sample_project();
        let out = render(&p, None, Timestamp::now());
        assert!(!out.contains("session"));
    }
}
