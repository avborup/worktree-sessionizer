use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use jiff::Timestamp;
use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::state::WorktreeEntry;
use crate::tmux;
use crate::ui::{Age, icons};

const CYAN: Color = Color::Cyan;
const MAGENTA: Color = Color::Magenta;
const DIM: Color = Color::DarkGray;
const WHITE: Color = Color::White;

#[tracing::instrument(level = "info", name = "switch.pick", skip(entries))]
pub fn pick(entries: &[WorktreeEntry]) -> Result<Option<WorktreeEntry>> {
    let sessions: Vec<bool> = tracing::info_span!("tmux.session_exists_scan", len = entries.len(),)
        .in_scope(|| {
            entries
                .iter()
                .map(|e| tmux::session_exists(&e.session_name))
                .collect()
        });

    crossterm::execute!(std::io::stderr(), EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(std::io::stderr());
    let mut terminal = Terminal::new(backend)?;

    let result = run_picker(&mut terminal, entries, &sessions);

    terminal::disable_raw_mode()?;
    crossterm::execute!(std::io::stderr(), LeaveAlternateScreen)?;

    result
}

struct PickerState {
    input: String,
    list_state: ListState,
    filtered: Vec<(usize, u16)>,
}

fn run_picker(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stderr>>,
    entries: &[WorktreeEntry],
    sessions: &[bool],
) -> Result<Option<WorktreeEntry>> {
    let mut state = PickerState {
        input: String::new(),
        list_state: ListState::default().with_selected(Some(0)),
        filtered: entries.iter().enumerate().map(|(i, _)| (i, 0)).collect(),
    };

    loop {
        let now = Timestamp::now();
        terminal.draw(|f| draw(f, entries, sessions, &mut state, now))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Enter => {
                    let sel = state.list_state.selected().unwrap_or(0);
                    return Ok(state
                        .filtered
                        .get(sel)
                        .map(|(idx, _)| entries[*idx].clone()));
                }
                KeyCode::Up => {
                    let i = state.list_state.selected().unwrap_or(0);
                    state.list_state.select(Some(i.saturating_sub(1)));
                }
                KeyCode::Down => {
                    let i = state.list_state.selected().unwrap_or(0);
                    let max = state.filtered.len().saturating_sub(1);
                    state
                        .list_state
                        .select(Some(i.min(max).wrapping_add(1).min(max)));
                }
                KeyCode::Backspace => {
                    state.input.pop();
                    refilter(entries, &mut state);
                }
                KeyCode::Char(c) => {
                    state.input.push(c);
                    refilter(entries, &mut state);
                }
                _ => {}
            }
        }
    }
}

fn refilter(entries: &[WorktreeEntry], state: &mut PickerState) {
    if state.input.is_empty() {
        state.filtered = entries.iter().enumerate().map(|(i, _)| (i, 0)).collect();
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
        state.filtered = entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let haystack = format!("{} {} {}", e.project, e.branch, e.session_name);
                let utf32 = Utf32Str::new(&haystack, &mut buf);
                atom.score(utf32, &mut matcher).map(|score| (i, score))
            })
            .collect();
        state.filtered.sort_by(|a, b| b.1.cmp(&a.1));
    }
    state.list_state.select(Some(0));
}

fn draw(
    f: &mut Frame,
    entries: &[WorktreeEntry],
    sessions: &[bool],
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

    let session_col_w = session_column_width(entries, &state.filtered);
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .map(|(idx, _)| build_row(entries, sessions, *idx, now, session_col_w))
        .collect();

    let count = state.filtered.len();
    let total = entries.len();
    let title = if state.input.is_empty() {
        format!(" worktrees ({total}) ")
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

/// Width cap for the session-name column; longer names are truncated.
const SESSION_COL_MAX: usize = 35;

fn session_column_width(entries: &[WorktreeEntry], filtered: &[(usize, u16)]) -> usize {
    filtered
        .iter()
        .map(|(idx, _)| entries[*idx].session_name.chars().count())
        .max()
        .unwrap_or(0)
        .min(SESSION_COL_MAX)
}

fn align_session_name(name: &str, width: usize) -> String {
    let truncated: String = name.chars().take(width).collect();
    format!("{:<width$}", truncated, width = width)
}

fn build_row(
    entries: &[WorktreeEntry],
    sessions: &[bool],
    idx: usize,
    now: Timestamp,
    session_col_w: usize,
) -> ListItem<'static> {
    let e = &entries[idx];
    let alive = sessions[idx];
    let age = Age::from_timestamps(now, e.last_used_at);
    let age_color = age.bracket().ratatui_color();

    let status_icon = if alive {
        "  ".to_string()
    } else {
        icons::INACTIVE.to_string()
    };
    let status_color = if alive { Color::Green } else { DIM };

    let spans = vec![
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::styled(
            format!("{:<3}  ", age.label()),
            Style::default().fg(age_color),
        ),
        Span::styled(format!("{}  ", e.project), Style::default().fg(MAGENTA)),
        Span::styled(
            format!("{} ", align_session_name(&e.session_name, session_col_w)),
            Style::default().fg(WHITE),
        ),
        Span::styled(icons::BRANCH, Style::default().fg(DIM)),
        Span::styled(e.branch.clone(), Style::default().fg(DIM)),
    ];

    ListItem::new(Line::from(spans))
}
