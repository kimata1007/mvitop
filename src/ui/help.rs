use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render(frame: &mut Frame<'_>, area: Rect) {
    let text = "Navigation\n  ↑/↓, j       select process       PgUp/PgDn  page\n  Enter        process detail       Esc        close overlay\n\nGPU process view\n  g / p        sort GPU time/PID    /          filter\n  Space        mark process         k          signal/kill menu\n  r            UI refresh rate\n\nApplication\n  ? / h        this help             q          quit\n\nOnly processes with GPU time in the latest 1s sample are listed.\nGPU process time comes from privileged macOS powermetrics data.";
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Help ")
                    .title_style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .borders(Borders::ALL),
            ),
        area,
    );
}
