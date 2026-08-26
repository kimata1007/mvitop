use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render(frame: &mut Frame<'_>, area: Rect) {
    let text = "Navigation\n  ↑/↓, j       select job           PgUp/PgDn  page\n  Enter        job detail           Esc        close overlay\n\nActive user jobs\n  g / p        sort GPU+CPU/PID     /          filter\n  Space        mark job             k          signal/kill menu\n  r            UI refresh rate\n\nApplication\n  ? / h        this help             q          quit\n\nForeground commands started from an interactive terminal are listed.\nChild CPU, GPU time, and unified memory are aggregated into the root job.\nGPU time comes from privileged macOS powermetrics data.";
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
