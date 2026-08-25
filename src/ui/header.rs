use crate::model::Snapshot;
use crate::ui::duration;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot) {
    let system = &snapshot.system;
    let title = Line::from(vec![
        Span::styled(
            " mvitop ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {}  ",
            if system.soc.is_empty() {
                "Apple Silicon"
            } else {
                &system.soc
            }
        )),
    ]);
    let line = if system.model.is_empty() {
        "collectors starting…".to_owned()
    } else {
        format!(
            "{}  macOS {}  uptime {}  load {:.2} {:.2} {:.2}",
            system.model,
            system.os_version,
            duration(system.uptime),
            system.load_average[0],
            system.load_average[1],
            system.load_average[2]
        )
    };
    frame.render_widget(
        Paragraph::new(line).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}
