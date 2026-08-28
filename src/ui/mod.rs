pub mod device;
pub mod graphs;
pub mod header;
pub mod help;
pub mod process_detail;
pub mod processes;

use crate::metrics::process::visible;
use crate::model::{Screen, Snapshot, ViewState};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutMode {
    Full,
    Compact,
    Minimal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MainLayout {
    mode: LayoutMode,
    header: Rect,
    meters: Rect,
    histories: Option<Rect>,
    processes: Rect,
    footer: Rect,
}

pub fn render(frame: &mut Frame<'_>, snapshot: &Snapshot, view: &ViewState) {
    let area = frame.area();
    let process_count = visible(&snapshot.processes, &view.filter, view.sort_key).len();
    let layout = main_layout(area, process_count);
    header::render(frame, layout.header, snapshot);
    device::render(frame, layout.meters, snapshot);
    if let Some(histories) = layout.histories {
        graphs::render(frame, histories, snapshot, layout.mode);
    }
    processes::render(frame, layout.processes, snapshot, view, layout.mode);
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
        layout.footer,
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

fn main_layout(area: Rect, process_count: usize) -> MainLayout {
    let mode = layout_mode(area);
    let footer_height = u16::from(area.height > 0);

    if mode == LayoutMode::Minimal {
        let header_height = match area.height {
            0..=4 => 1,
            5..=9 => 2,
            _ => 3,
        };
        let meter_height = match area.height {
            0..=3 => 0,
            4..=9 => 3,
            _ => 5,
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Length(meter_height),
                Constraint::Min(0),
                Constraint::Length(footer_height),
            ])
            .split(area);
        return MainLayout {
            mode,
            header: chunks[0],
            meters: chunks[1],
            histories: None,
            processes: chunks[2],
            footer: chunks[3],
        };
    }

    let header_height = 3;
    let meter_height = 5;
    let graph_minimum = match mode {
        LayoutMode::Full => 12,
        LayoutMode::Compact => 9,
        LayoutMode::Minimal => unreachable!(),
    };
    let process_maximum = match mode {
        LayoutMode::Full => 10,
        LayoutMode::Compact => 6,
        LayoutMode::Minimal => unreachable!(),
    };
    let desired_process_height = if process_count == 0 {
        3
    } else {
        (process_count as u16)
            .saturating_add(3)
            .clamp(4, process_maximum)
    };
    let available_process_height = area
        .height
        .saturating_sub(header_height + meter_height + graph_minimum + footer_height);
    let process_height = desired_process_height.min(available_process_height);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(meter_height),
            Constraint::Min(graph_minimum),
            Constraint::Length(process_height),
            Constraint::Length(footer_height),
        ])
        .split(area);
    MainLayout {
        mode,
        header: chunks[0],
        meters: chunks[1],
        histories: Some(chunks[2]),
        processes: chunks[3],
        footer: chunks[4],
    }
}

fn layout_mode(area: Rect) -> LayoutMode {
    if area.width < 50 || area.height < 22 {
        LayoutMode::Minimal
    } else if area.height < 34 {
        LayoutMode::Compact
    } else {
        LayoutMode::Full
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

    #[test]
    fn prioritizes_metrics_over_the_job_table() {
        let large = main_layout(Rect::new(0, 0, 120, 40), 0);
        assert_eq!(large.mode, LayoutMode::Full);
        assert_eq!(large.meters.height, 5);
        assert!(large.histories.unwrap().height >= 12);
        assert_eq!(large.processes.height, 3);

        let compact = main_layout(Rect::new(0, 0, 80, 24), 12);
        assert_eq!(compact.mode, LayoutMode::Compact);
        assert_eq!(compact.histories.unwrap().height, 9);
        assert_eq!(compact.processes.height, 6);

        let minimal = main_layout(Rect::new(0, 0, 60, 20), 0);
        assert_eq!(minimal.mode, LayoutMode::Minimal);
        assert!(minimal.histories.is_none());
    }

    #[test]
    fn empty_job_panel_stays_collapsed() {
        let empty = main_layout(Rect::new(0, 0, 120, 40), 0);
        let busy = main_layout(Rect::new(0, 0, 120, 40), 20);
        assert_eq!(empty.processes.height, 3);
        assert_eq!(busy.processes.height, 10);
        assert!(empty.histories.unwrap().height > busy.histories.unwrap().height);
    }
}
