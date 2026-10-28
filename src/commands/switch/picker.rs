use std::cmp::Reverse;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use jiff::Timestamp;
use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::commands::switch::SwitchTarget;
use crate::state::WorktreeEntry;
use crate::tmux;
use crate::ui::{Age, icons};

const CYAN: Color = Color::Cyan;
const MAGENTA: Color = Color::Magenta;
const DIM: Color = Color::DarkGray;
const WHITE: Color = Color::White;

const SESSION_REFRESH_POLL: Duration = Duration::from_millis(50);
const SESSION_COL_MAX: usize = 35;
const PROJECT_COL_MAX: usize = 28;

#[tracing::instrument(level = "info", name = "switch.pick", skip(targets))]
pub fn pick(targets: &[SwitchTarget]) -> Result<Option<SwitchTarget>> {
    let worktrees: Vec<WorktreeEntry> = targets
        .iter()
        .filter_map(|t| match t {
            SwitchTarget::Worktree(e) => Some(e.clone()),
            SwitchTarget::Session(_) => None,
        })
        .collect();
    let mut presence = tracing::info_span!("tmux.session_presence", len = worktrees.len())
        .in_scope(|| tmux::prepare_picker_presence(&worktrees));

    stderr_ui_begin()?;

    let backend = CrosstermBackend::new(std::io::stderr());
    let mut terminal = Terminal::new(backend)?;
    let result = run_picker(&mut terminal, targets, &mut presence);

    stderr_ui_end()?;

    result
}

fn stderr_ui_begin() -> Result<()> {
    crossterm::execute!(std::io::stderr(), EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    Ok(())
}

fn stderr_ui_end() -> Result<()> {
    terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), LeaveAlternateScreen)?;
    Ok(())
}

struct PickerState {
    input: String,
    list_state: ListState,
    filtered: Vec<(usize, u16)>,
}

impl PickerState {
    fn new(targets: &[SwitchTarget]) -> Self {
        Self {
            input: String::new(),
            list_state: ListState::default().with_selected(Some(0)),
            filtered: targets.iter().enumerate().map(|(i, _)| (i, 0)).collect(),
        }
    }
}

enum PickAction {
    Continue,
    Cancel,
    Choose(Option<SwitchTarget>),
}

fn worktree_count(targets: &[SwitchTarget]) -> usize {
    targets.iter().filter(|t| t.worktree().is_some()).count()
}

fn is_alive(targets: &[SwitchTarget], presence: &tmux::PickerPresence, idx: usize) -> bool {
    match &targets[idx] {
        SwitchTarget::Worktree(_) => presence.alive.get(idx).copied().unwrap_or(false),
        SwitchTarget::Session(_) => true,
    }
}

fn drain_alive_updates(presence: &mut tmux::PickerPresence, worktrees: usize) {
    let Some(rx) = presence.updates.as_ref() else {
        return;
    };
    while let Ok(next) = rx.try_recv() {
        if next.len() == worktrees {
            presence.alive = next;
        }
    }
}

/// Blocks until a key event. While waiting, merges tmux refresh payloads and redraws.
fn wait_polled_key_event(
    presence: &mut tmux::PickerPresence,
    worktrees: usize,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stderr>>,
    targets: &[SwitchTarget],
    state: &mut PickerState,
) -> Result<Event> {
    loop {
        if event::poll(SESSION_REFRESH_POLL)? {
            return Ok(event::read()?);
        }
        drain_alive_updates(presence, worktrees);
        let now = Timestamp::now();
        terminal.draw(|f| draw(f, targets, presence, state, now))?;
    }
}

fn run_picker(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stderr>>,
    targets: &[SwitchTarget],
    presence: &mut tmux::PickerPresence,
) -> Result<Option<SwitchTarget>> {
    let worktrees = worktree_count(targets);
    let mut state = PickerState::new(targets);

    loop {
        drain_alive_updates(presence, worktrees);

        let now = Timestamp::now();
        terminal.draw(|f| draw(f, targets, presence, &mut state, now))?;

        let event = if presence.updates.is_some() {
            wait_polled_key_event(presence, worktrees, terminal, targets, &mut state)?
        } else {
            event::read()?
        };

        let Event::Key(key) = event else {
            continue;
        };

        match dispatch_pick_key(key, targets, &mut state) {
            PickAction::Continue => {}
            PickAction::Cancel => return Ok(None),
            PickAction::Choose(entry) => return Ok(entry),
        }
    }
}

fn dispatch_pick_key(
    key: KeyEvent,
    targets: &[SwitchTarget],
    state: &mut PickerState,
) -> PickAction {
    if key.kind != KeyEventKind::Press {
        return PickAction::Continue;
    }

    match key.code {
        KeyCode::Esc => PickAction::Cancel,
        KeyCode::Enter => PickAction::Choose(selected_target(targets, state)),
        KeyCode::Up => {
            let i = state.list_state.selected().unwrap_or(0);
            state.list_state.select(Some(i.saturating_sub(1)));
            PickAction::Continue
        }
        KeyCode::Down => {
            let i = state.list_state.selected().unwrap_or(0);
            let max = state.filtered.len().saturating_sub(1);
            state
                .list_state
                .select(Some(i.min(max).wrapping_add(1).min(max)));
            PickAction::Continue
        }
        KeyCode::Backspace => {
            state.input.pop();
            refilter(targets, state);
            PickAction::Continue
        }
        KeyCode::Char(c) => {
            state.input.push(c);
            refilter(targets, state);
            PickAction::Continue
        }
        _ => PickAction::Continue,
    }
}

fn selected_target(targets: &[SwitchTarget], state: &PickerState) -> Option<SwitchTarget> {
    let sel = state.list_state.selected().unwrap_or(0);
    state
        .filtered
        .get(sel)
        .map(|(idx, _)| targets[*idx].clone())
}

fn refilter(targets: &[SwitchTarget], state: &mut PickerState) {
    if state.input.is_empty() {
        state.filtered = targets.iter().enumerate().map(|(i, _)| (i, 0)).collect();
    } else {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let atom = Atom::new(
            &state.input,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );
        let mut buf = Vec::new();
        state.filtered = targets
            .iter()
            .enumerate()
            .filter_map(|(i, target)| {
                let haystack = target.fuzzy_text();
                let utf32 = Utf32Str::new(&haystack, &mut buf);
                atom.score(utf32, &mut matcher).map(|score| (i, score))
            })
            .collect();
        state.filtered.sort_by_key(|&(_, score)| Reverse(score));
    }
    state.list_state.select(Some(0));
}

fn draw(
    f: &mut Frame,
    targets: &[SwitchTarget],
    presence: &tmux::PickerPresence,
    state: &mut PickerState,
    now: Timestamp,
) {
    let area = f.area();
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CYAN))
        .title(Span::styled(
            " filter ".to_string(),
            Style::default().fg(CYAN).bold(),
        ));
    let input = Paragraph::new(Span::styled(
        state.input.as_str(),
        Style::default().fg(WHITE),
    ))
    .block(input_block);
    f.render_widget(input, chunks[0]);

    f.set_cursor_position((chunks[0].x + state.input.len() as u16 + 1, chunks[0].y + 1));

    let project_col_w = capped_column_width(
        targets,
        &state.filtered,
        PROJECT_COL_MAX,
        project_field_width,
    );
    let session_col_w = capped_column_width(targets, &state.filtered, SESSION_COL_MAX, |t| {
        t.session_name().chars().count()
    });
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|(idx, _)| build_row(targets, presence, *idx, now, project_col_w, session_col_w))
        .collect();

    let count = state.filtered.len();
    let total = targets.len();
    let has_sessions = targets
        .iter()
        .any(|t| matches!(t, SwitchTarget::Session(_)));
    let title = if state.input.is_empty() {
        if has_sessions {
            format!(" worktrees + sessions ({total}) ")
        } else {
            format!(" worktrees ({total}) ")
        }
    } else if has_sessions {
        format!(" worktrees + sessions ({count}/{total}) ")
    } else {
        format!(" worktrees ({count}/{total}) ")
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM))
                .title(Span::styled(title, Style::default().fg(WHITE).bold())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, chunks[1], &mut state.list_state);

    let help = Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(CYAN).bold()),
        Span::styled(" navigate  ", Style::default().fg(DIM)),
        Span::styled("⏎", Style::default().fg(CYAN).bold()),
        Span::styled(" select  ", Style::default().fg(DIM)),
        Span::styled("esc", Style::default().fg(CYAN).bold()),
        Span::styled(" cancel", Style::default().fg(DIM)),
    ]);
    f.render_widget(Paragraph::new(help), chunks[2]);
}

fn project_field_width(target: &SwitchTarget) -> usize {
    match target {
        SwitchTarget::Worktree(e) => e.project.chars().count(),
        SwitchTarget::Session(_) => 1,
    }
}

fn capped_column_width(
    targets: &[SwitchTarget],
    filtered: &[(usize, u16)],
    cap: usize,
    mut measure: impl FnMut(&SwitchTarget) -> usize,
) -> usize {
    filtered
        .iter()
        .map(|(idx, _)| measure(&targets[*idx]))
        .max()
        .unwrap_or(0)
        .min(cap)
}

fn align_fixed_field(value: &str, width: usize) -> String {
    let truncated: String = value.chars().take(width).collect();
    format!("{:<width$}", truncated, width = width)
}

fn build_row(
    targets: &[SwitchTarget],
    presence: &tmux::PickerPresence,
    idx: usize,
    now: Timestamp,
    project_col_w: usize,
    session_col_w: usize,
) -> ListItem<'static> {
    match &targets[idx] {
        SwitchTarget::Worktree(e) => build_worktree_row(
            e,
            is_alive(targets, presence, idx),
            now,
            project_col_w,
            session_col_w,
        ),
        SwitchTarget::Session(name) => build_session_row(name, project_col_w, session_col_w),
    }
}

fn build_worktree_row(
    e: &WorktreeEntry,
    is_alive: bool,
    now: Timestamp,
    project_col_w: usize,
    session_col_w: usize,
) -> ListItem<'static> {
    let age = Age::from_timestamps(now, e.last_used_at);
    let age_color = age.bracket().ratatui_color();

    let status_icon = if is_alive {
        "  ".to_string()
    } else {
        icons::INACTIVE.to_string()
    };
    let status_color = if is_alive { Color::Green } else { DIM };

    let spans = vec![
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::styled(
            format!("{:<3}  ", age.label()),
            Style::default().fg(age_color),
        ),
        Span::styled(
            format!("{} ", align_fixed_field(&e.project, project_col_w)),
            Style::default().fg(MAGENTA),
        ),
        Span::styled(
            format!("{} ", align_fixed_field(&e.session_name, session_col_w)),
            Style::default().fg(WHITE),
        ),
        Span::styled(icons::BRANCH, Style::default().fg(DIM)),
        Span::styled(e.branch.clone(), Style::default().fg(DIM)),
    ];

    ListItem::new(Line::from(spans))
}

fn build_session_row(name: &str, project_col_w: usize, session_col_w: usize) -> ListItem<'static> {
    let spans = vec![
        Span::styled("  ", Style::default().fg(Color::Green)),
        Span::styled("·    ", Style::default().fg(DIM)),
        Span::styled(
            format!("{} ", align_fixed_field("—", project_col_w)),
            Style::default().fg(DIM),
        ),
        Span::styled(
            format!("{} ", align_fixed_field(name, session_col_w)),
            Style::default().fg(WHITE),
        ),
    ];

    ListItem::new(Line::from(spans))
}
