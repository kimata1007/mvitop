use crate::metrics::process::visible;
use crate::model::{Snapshot, SortKey, ViewState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Row, Table, TableState};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot, view: &ViewState) {
    let items = visible(&snapshot.processes, &view.filter, view.sort_key);
    let widths = [
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(14),
        Constraint::Min(12),
    ];
    let rows: Vec<_> = if items.is_empty() {
        let message = snapshot
            .status
            .process_error
            .as_deref()
            .unwrap_or("No GPU activity in the latest 1s sample");
        vec![Row::new(["", "", "", message]).style(Style::default().fg(
            if snapshot.status.process_error.is_some() {
                Color::Yellow
            } else {
                Color::DarkGray
            },
        ))]
    } else {
        items
            .iter()
            .map(|process| {
                Row::new([
                    if view.marked.contains(&process.pid) {
                        "*".to_owned()
                    } else {
                        " ".to_owned()
                    },
                    process.pid.to_string(),
                    format!("{:.1} ms/s", process.gpu_time_ms_per_s),
                    process.command.clone(),
                ])
                .style(if view.marked.contains(&process.pid) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
            })
            .collect()
    };
    let sort = match view.sort_key {
        SortKey::Gpu => "GPU time",
        SortKey::Pid => "PID",
    };
    let filter = if view.filter.is_empty() {
        String::new()
    } else {
        format!("  filter:{}", view.filter)
    };
    let title = format!(
        " GPU Active Processes {}  sort:{sort}{filter} ",
        items.len()
    );
    let table = Table::new(rows, widths)
        .header(
            Row::new([" ", "PID", "GPU TIME", "COMMAND"]).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    let mut state = TableState::default().with_selected(if items.is_empty() {
        None
    } else {
        Some(view.selected.min(items.len() - 1))
    });
    frame.render_stateful_widget(table, area, &mut state);
}
