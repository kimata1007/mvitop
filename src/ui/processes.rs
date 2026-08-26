use crate::metrics::process::visible;
use crate::model::{Snapshot, SortKey, ViewState};
use crate::ui::bytes;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Row, Table, TableState};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot, view: &ViewState) {
    let items = visible(&snapshot.processes, &view.filter, view.sort_key);
    let widths = [
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Min(12),
    ];
    let rows: Vec<_> = if items.is_empty() {
        let message = snapshot
            .status
            .process_error
            .as_deref()
            .unwrap_or("No active foreground user jobs");
        vec![
            Row::new(["", "", "", "", "", message]).style(Style::default().fg(
                if snapshot.status.process_error.is_some() {
                    Color::Yellow
                } else {
                    Color::DarkGray
                },
            )),
        ]
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
                    gpu_time(snapshot, process.gpu_time_ms_per_s),
                    if process.cpu_percent >= 0.05 {
                        format!("{:.1}%", process.cpu_percent)
                    } else {
                        "—".into()
                    },
                    bytes(process.memory_bytes),
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
        SortKey::Gpu => "GPU/CPU",
        SortKey::Pid => "PID",
    };
    let filter = if view.filter.is_empty() {
        String::new()
    } else {
        format!("  filter:{}", view.filter)
    };
    let title = format!(" ACTIVE USER JOBS {}  sort:{sort}{filter} ", items.len());
    let table = Table::new(rows, widths)
        .header(
            Row::new([" ", "PID", "GPU TIME", "CPU", "UMEM", "COMMAND"]).style(
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

fn gpu_time(snapshot: &Snapshot, value: f64) -> String {
    if snapshot.status.gpu_process_error.is_some() {
        "N/A".into()
    } else if value > 0.0 {
        format!("{value:.1} ms/s")
    } else {
        "—".into()
    }
}
