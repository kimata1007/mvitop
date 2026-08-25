pub mod device;
pub mod graphs;
pub mod header;
pub mod help;
pub mod process_detail;
pub mod processes;

use crate::model::{Screen, Snapshot, ViewState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn render(frame: &mut Frame<'_>, snapshot: &Snapshot, view: &ViewState) {
    let area = frame.area();
    let show_graphs = area.height >= 28 && area.width >= 60;
    let rows = if show_graphs {
        vec![
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(7),
            Constraint::Min(5),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(1),
        ]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(rows)
        .split(area);
    header::render(frame, chunks[0], snapshot);
    device::render(frame, chunks[1], snapshot);
    let (process_area, footer_index) = if show_graphs {
        graphs::render(frame, chunks[2], snapshot);
        (chunks[3], 4)
    } else {
        (chunks[2], 3)
    };
    processes::render(frame, process_area, snapshot, view);
    let message = if view.editing_filter {
        format!(" filter: {}_", view.filter)
    } else if let Some(message) = &view.message {
        format!(" {message}")
    } else {
        format!(
            " q quit  ? help  / filter  sort[g/p]  detail[Enter]  signal[k]  refresh[r] {}ms",
            refresh_millis(view)
        )
    };
    frame.render_widget(
        ratatui::widgets::Paragraph::new(message)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray)),
        chunks[footer_index],
    );

    match view.screen {
        Screen::Help => help::render(frame, centered(area, 66, 24)),
        Screen::Detail => {
            process_detail::render_detail(frame, centered(area, 76, 23), snapshot, view)
        }
        Screen::Signal => process_detail::render_signal(frame, centered(area, 62, 14), view),
        Screen::Main => {}
    }
}

pub fn refresh_millis(view: &ViewState) -> u64 {
    [100, 200, 500, 1_000][view.refresh_rate_index % 4]
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(1);
    let height = height.min(area.height.saturating_sub(2)).max(1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = value as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

pub fn duration(value: std::time::Duration) -> String {
    let seconds = value.as_secs();
    let days = seconds / 86_400;
    let hours = seconds / 3_600 % 24;
    let minutes = seconds / 60 % 60;
    if days > 0 {
        format!("{days}d {hours:02}:{minutes:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{:02}", seconds % 60)
    }
}

pub fn system_time(value: std::time::SystemTime) -> String {
    let Ok(duration) = value.duration_since(std::time::UNIX_EPOCH) else {
        return "N/A".into();
    };
    let seconds = duration.as_secs() as libc::time_t;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: localtime_r initializes the provided tm for a valid time_t.
    if unsafe { libc::localtime_r(&seconds, broken_down.as_mut_ptr()) }.is_null() {
        return duration.as_secs().to_string();
    }
    let format = c"%Y-%m-%d %H:%M:%S";
    let mut buffer = [0i8; 32];
    // SAFETY: tm was initialized and the output buffer is writable.
    let length = unsafe {
        libc::strftime(
            buffer.as_mut_ptr(),
            buffer.len(),
            format.as_ptr(),
            broken_down.as_ptr(),
        )
    };
    if length == 0 {
        return duration.as_secs().to_string();
    }
    // SAFETY: strftime writes ASCII and NUL-terminates it.
    unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    #[test]
    fn formats_values_compactly() {
        assert_eq!(bytes(1_073_741_824), "1.0GiB");
        assert_eq!(duration(std::time::Duration::from_secs(3661)), "01:01:01");
    }

    #[test]
    fn renders_responsive_layouts_and_overlays() {
        for (width, height) in [(30, 8), (60, 20), (80, 24), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            for screen in [Screen::Main, Screen::Help, Screen::Detail, Screen::Signal] {
                let view = ViewState {
                    screen,
                    signal_targets: vec![(42, None)],
                    ..ViewState::default()
                };
                terminal
                    .draw(|frame| render(frame, &Snapshot::default(), &view))
                    .unwrap();
            }
        }
    }
}
