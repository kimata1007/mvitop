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
        " GPU % ",
        &snapshot.gpu.history,
        Color::Magenta,
    );
    render_one(
        frame,
        columns[1],
        " CPU % ",
        &snapshot.cpu.history,
        Color::Cyan,
    );
    render_one(
        frame,
        columns[2],
        " Memory % ",
        &snapshot.memory.history,
        Color::Green,
    );
}

fn render_one(frame: &mut Frame<'_>, area: Rect, title: &str, history: &History, color: Color) {
    let data: Vec<u64> = history.iter().map(|value| value.round() as u64).collect();
    let title = if history.is_empty() {
        format!("{title} waiting ")
    } else {
        format!("{title} {} samples ", history.len())
    };
    let graph = Sparkline::default()
        .block(Block::default().title(title).borders(Borders::ALL))
        .data(&data)
        .max(100)
        .style(Style::default().fg(color));
    frame.render_widget(graph, area);
}
