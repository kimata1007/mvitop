use crate::metrics::process::visible;
use crate::model::{ProcessInfo, Snapshot, SortKey, ViewState};
use crate::ui::{LayoutMode, bytes, duration};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TableDensity {
    Full,
    Medium,
    Narrow,
    Tiny,
}

struct TableSchema {
    density: TableDensity,
    widths: Vec<Constraint>,
    headers: Vec<&'static str>,
}

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    snapshot: &Snapshot,
    view: &ViewState,
    mode: LayoutMode,
) {
    if area.is_empty() {
        return;
    }
    let items = visible(&snapshot.processes, &view.filter, view.sort_key);
    let title = panel_title(snapshot, view, items.len(), area.width);
    if items.is_empty() {
        let message = snapshot
            .status
            .process_error
            .as_deref()
            .unwrap_or("No active foreground user jobs");
        frame.render_widget(
            Paragraph::new(format!(" {message}"))
                .style(
                    Style::default().fg(if snapshot.status.process_error.is_some() {
                        Color::Yellow
                    } else {
                        Color::DarkGray
                    }),
                )
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
            area,
        );
        return;
    }

    let schema = table_schema(mode, area.width);
    let rows = items
        .iter()
        .map(|process| process_row(snapshot, view, process, schema.density));
    let table = Table::new(rows, schema.widths)
        .header(
            Row::new(schema.headers).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .column_spacing(1)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    let mut state = TableState::default().with_selected(Some(view.selected.min(items.len() - 1)));
    frame.render_stateful_widget(table, area, &mut state);
}

fn table_schema(mode: LayoutMode, width: u16) -> TableSchema {
    if mode == LayoutMode::Full && width >= 105 {
        TableSchema {
            density: TableDensity::Full,
            widths: vec![
                Constraint::Length(1),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(15),
            ],
            headers: vec!["", "PID", "GPU ms/s", "CPU", "UMEM", "TIME", "COMMAND"],
        }
    } else if width >= 78 {
        TableSchema {
            density: TableDensity::Medium,
            widths: vec![
                Constraint::Length(1),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Min(12),
            ],
            headers: vec!["", "PID", "GPU ms/s", "CPU", "UMEM", "COMMAND"],
        }
    } else if width >= 54 {
        TableSchema {
            density: TableDensity::Narrow,
            widths: vec![
                Constraint::Length(1),
                Constraint::Length(7),
                Constraint::Length(10),
                Constraint::Length(7),
                Constraint::Min(10),
            ],
            headers: vec!["", "PID", "GPU ms/s", "CPU", "COMMAND"],
        }
    } else {
        TableSchema {
            density: TableDensity::Tiny,
            widths: vec![
                Constraint::Length(1),
                Constraint::Length(7),
                Constraint::Length(8),
                Constraint::Min(8),
            ],
            headers: vec!["", "PID", "ACTIVITY", "COMMAND"],
        }
    }
}

fn process_row(
    snapshot: &Snapshot,
    view: &ViewState,
    process: &ProcessInfo,
    density: TableDensity,
) -> Row<'static> {
    let marker = if view.marked.contains(&process.pid) {
        "*"
    } else {
        " "
    };
    let cpu = if process.cpu_percent >= 0.05 {
        format!("{:.1}%", process.cpu_percent)
    } else {
        "—".into()
    };
    let cells = match density {
        TableDensity::Full => vec![
            marker.into(),
            process.pid.to_string(),
            gpu_time(snapshot, process.gpu_time_ms_per_s),
            cpu,
            bytes(process.memory_bytes),
            duration(process.runtime),
            process.command.clone(),
        ],
        TableDensity::Medium => vec![
            marker.into(),
            process.pid.to_string(),
            gpu_time(snapshot, process.gpu_time_ms_per_s),
            cpu,
            bytes(process.memory_bytes),
            process.command.clone(),
        ],
        TableDensity::Narrow => vec![
            marker.into(),
            process.pid.to_string(),
            gpu_time(snapshot, process.gpu_time_ms_per_s),
            cpu,
            process.command.clone(),
        ],
        TableDensity::Tiny => vec![
            marker.into(),
            process.pid.to_string(),
            activity(snapshot, process),
            process.command.clone(),
        ],
    };
    Row::new(cells).style(if view.marked.contains(&process.pid) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    })
}

fn panel_title(snapshot: &Snapshot, view: &ViewState, count: usize, width: u16) -> String {
    let sort = match view.sort_key {
        SortKey::Gpu => "GPU▼",
        SortKey::Pid => "PID▲",
    };
    let filter = if view.filter.is_empty() {
        String::new()
    } else {
        format!(" filter:{}", view.filter)
    };
    let gpu_status = if snapshot.status.gpu_process_error.is_some() {
        " GPU:N/A"
    } else {
        ""
    };
    if width >= 60 {
        format!(" ACTIVE USER JOBS {count}  sort:{sort}{filter}{gpu_status} ")
    } else {
        format!(" JOBS {count} {sort}{filter}{gpu_status} ")
    }
}

fn gpu_time(snapshot: &Snapshot, value: f64) -> String {
    if snapshot.status.gpu_process_error.is_some() {
        "N/A".into()
    } else if value > 0.0 {
        format!("{value:.1}")
    } else {
        "—".into()
    }
}

fn activity(snapshot: &Snapshot, process: &ProcessInfo) -> String {
    if snapshot.status.gpu_process_error.is_none() && process.gpu_time_ms_per_s > 0.0 {
        format!("G{:.0}", process.gpu_time_ms_per_s)
    } else if process.cpu_percent >= 0.05 {
        format!("C{:.1}%", process.cpu_percent)
    } else {
        "—".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_columns_follow_available_width() {
        let full = table_schema(LayoutMode::Full, 120);
        assert_eq!(full.density, TableDensity::Full);
        assert!(full.headers.contains(&"TIME"));
        assert!(full.headers.contains(&"GPU ms/s"));

        let compact = table_schema(LayoutMode::Compact, 80);
        assert_eq!(compact.density, TableDensity::Medium);
        assert!(!compact.headers.contains(&"TIME"));

        let tiny = table_schema(LayoutMode::Minimal, 30);
        assert_eq!(tiny.density, TableDensity::Tiny);
        assert_eq!(tiny.headers, vec!["", "PID", "ACTIVITY", "COMMAND"]);
    }

    #[test]
    fn gpu_values_rely_on_the_header_for_units() {
        let snapshot = Snapshot::default();
        assert_eq!(gpu_time(&snapshot, 12.25), "12.2");
        assert_eq!(gpu_time(&snapshot, 0.0), "—");
    }
}
