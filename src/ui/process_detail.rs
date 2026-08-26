use crate::metrics::process::visible;
use crate::model::{Snapshot, ViewState};
use crate::ui::{bytes, duration};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

pub fn render_detail(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot, view: &ViewState) {
    frame.render_widget(Clear, area);
    let items = visible(&snapshot.processes, &view.filter, view.sort_key);
    let process = view
        .selected_pid
        .and_then(|pid| snapshot.processes.iter().find(|process| process.pid == pid))
        .or_else(|| items.get(view.selected).copied());
    let content = process.map(|process| format!(
        "Job PID / PPID {} / {}\nProcesses       {}\nGPU time        {}\nExecutable      {}\nCommand         {}\nUser            {}\nState           {}\nCPU total       {:.1}%\nUnified memory  {} ({:.2}%)\nThreads total   {}\nRuntime         {}\nStart time      {}\nWorking dir     {}",
        process.pid, process.ppid, process.member_count, gpu_time(snapshot, process.gpu_time_ms_per_s), fallback(&process.executable), process.command, process.user, process.state.short(), process.cpu_percent, bytes(process.memory_bytes), process.memory_percent, process.threads, duration(process.runtime), process.start_time.map(crate::ui::system_time).unwrap_or_else(|| "N/A".into()), process.cwd.as_deref().unwrap_or("N/A (permission/API)")
    )).unwrap_or_else(|| "Process is no longer available.".to_owned());
    frame.render_widget(
        Paragraph::new(content).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" User job detail — Esc to close ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        ),
        area,
    );
}

pub fn render_signal(frame: &mut Frame<'_>, area: Rect, view: &ViewState) {
    frame.render_widget(Clear, area);
    let signals = [
        ("SIGTERM (15)", "request graceful termination"),
        ("SIGINT (2)", "interrupt"),
        ("SIGKILL (9)", "force termination; cannot be handled"),
    ];
    let items = signals
        .iter()
        .enumerate()
        .map(|(index, (name, description))| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    if index == view.signal_index {
                        "▶ "
                    } else {
                        "  "
                    },
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(*name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("  {description}")),
            ]))
        });
    let title = format!(" Confirm signal to {} job(s) ", view.signal_targets.len());
    let footer = Paragraph::new("↑/↓ select · Enter sends · Esc cancels")
        .style(Style::default().fg(Color::Yellow));
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(4),
    };
    frame.render_widget(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
        area,
    );
    frame.render_widget(List::new(items), inner);
    frame.render_widget(
        footer,
        Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(2),
            width: area.width.saturating_sub(4),
            height: 1,
        },
    );
}

fn fallback(value: &str) -> &str {
    if value.is_empty() { "N/A" } else { value }
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
