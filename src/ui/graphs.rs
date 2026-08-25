use crate::model::{History, Snapshot};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Sparkline};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);
    render_one(
        frame,
        columns[0],
        &format!(" CPU UTIL {:.0}% ", snapshot.cpu.total_percent),
        &snapshot.cpu.history,
        Color::Cyan,
    );
    render_one(
        frame,
        columns[1],
        &format!(
            " GPU UTIL {} ",
            snapshot
                .gpu
                .utilization_percent
                .map(|value| format!("{value:.0}%"))
                .unwrap_or_else(|| "N/A".into())
        ),
        &snapshot.gpu.history,
        Color::Magenta,
    );
    render_one(
        frame,
        columns[2],
        &format!(
            " UNIFIED MEM {:.0}% ",
            snapshot.memory.used as f64 * 100.0 / snapshot.memory.total.max(1) as f64
        ),
        &snapshot.memory.history,
        Color::Green,
    );
}

fn render_one(frame: &mut Frame<'_>, area: Rect, title: &str, history: &History, color: Color) {
    let data: Vec<u64> = history.iter().map(|value| value.round() as u64).collect();
    let title = if history.is_empty() {
        format!("{title}waiting ")
    } else {
        title.to_owned()
    };
    let graph = Sparkline::default()
        .block(Block::default().title(title).borders(Borders::ALL))
        .data(&data)
        .max(100)
        .style(Style::default().fg(color));
    frame.render_widget(graph, area);
}
