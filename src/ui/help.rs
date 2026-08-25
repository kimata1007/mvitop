use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn render(frame: &mut Frame<'_>, area: Rect) {
    let text = "Navigation\n  ↑/↓, j       select process       PgUp/PgDn  page\n  Enter        process detail       Esc        close overlay\n\nProcess view\n  c / m / p    sort CPU/memory/PID  /          filter\n  t            toggle process tree  Space      mark process\n  k            signal/kill menu     r          UI refresh rate\n\nApplication\n  ? / h        this help             q          quit\n\nGPU values are shown only when the driver publishes a real value.\nPer-process GPU usage is not estimated.";
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
