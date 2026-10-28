use jiff::Timestamp;
use owo_colors::OwoColorize;
use wt::Step;

pub struct Age {
    pub seconds: i64,
}

impl Age {
    pub fn from_timestamps(now: Timestamp, then: Timestamp) -> Self {
        let seconds = now
            .since(then)
            .map(|s| i64::from(s.get_hours()) * 3600 + s.get_minutes() * 60 + s.get_seconds())
            .unwrap_or(0);
        Self { seconds }
    }

    pub fn days(&self) -> i64 {
        self.seconds / 86400
    }

    pub fn label(&self) -> String {
        let s = self.seconds;
        if s < 60 {
            format!("{}s", s)
        } else if s < 3600 {
            format!("{}m", s / 60)
        } else if s < 86400 {
            format!("{}h", s / 3600)
        } else {
            format!("{}d", s / 86400)
        }
    }

    pub fn bracket(&self) -> AgeBracket {
        match self.days() {
            0 => AgeBracket::Fresh,
            1..=7 => AgeBracket::Recent,
            8..=30 => AgeBracket::Aging,
            _ => AgeBracket::Stale,
        }
    }
}

#[derive(Clone, Copy)]
pub enum AgeBracket {
    Fresh,
    Recent,
    Aging,
    Stale,
}

impl AgeBracket {
    pub fn ratatui_color(self) -> ratatui::style::Color {
        match self {
            Self::Fresh => ratatui::style::Color::Green,
            Self::Recent => ratatui::style::Color::Yellow,
            Self::Aging => ratatui::style::Color::Rgb(255, 140, 0),
            Self::Stale => ratatui::style::Color::Red,
        }
    }

    pub fn paint(self, text: &str) -> String {
        match self {
            Self::Fresh => text.green().to_string(),
            Self::Recent => text.yellow().to_string(),
            Self::Aging => text.truecolor(255, 140, 0).to_string(),
            Self::Stale => text.red().to_string(),
        }
    }
}

pub fn step(message: &str) {
    eprintln!("  {} {}", "→".cyan(), message);
}

pub fn step_done(message: &str) {
    eprintln!("  {} {}", "✓".green(), message);
}

/// Prints a library step the way `wt new` and `wt remove` always have.
pub fn progress(event: Step) {
    match event {
        Step::Branching { branch, base } => step(&format!(
            "branching {} from {}",
            branch.white(),
            base.dimmed()
        )),
        Step::CheckingOut { branch } => step(&format!("checking out {}", branch.white())),
        Step::WorktreeAdded { path } => step_done(&format!("{}", path.display().dimmed())),
        Step::CowCloning { entry } => step(&format!("cow cloning {}", entry.cyan())),
        Step::Symlinking { entry } => step(&format!("symlinking {}", entry.cyan())),
        Step::RemovingWorktree { path } => {
            step(&format!("removing worktree at {}", path.display().dimmed()))
        }
        Step::DeletingBranch { branch, force } => {
            let label = if force { "force-deleting" } else { "deleting" };
            step(&format!("{} branch {}", label, branch.yellow()))
        }
        Step::SavingMetadata => step("saving metadata"),
    }
}

pub mod icons {
    pub const BRANCH: &str = "󰘬 ";
    pub const INACTIVE: &str = "󰒲 ";
}
