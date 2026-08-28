use crate::model::{History, Snapshot};
use crate::ui::bytes;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Gauge};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot) {
    if area.is_empty() {
        return;
    }
    let borders = if area.height >= 5 {
        Borders::TOP | Borders::BOTTOM
    } else {
        Borders::NONE
    };
    let block = Block::default()
        .borders(borders)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 3])
        .split(inner);
    if rows.len() < 3 {
        return;
    }
    let clusters = match (
        snapshot.system.performance_cores,
        snapshot.system.efficiency_cores,
    ) {
        (Some(p), Some(e)) => format!(" P:{p} E:{e}"),
        _ => String::new(),
    };
    let per_core = if area.width >= 100 && !snapshot.cpu.per_core_percent.is_empty() {
        let values = snapshot
            .cpu
            .per_core_percent
            .iter()
            .enumerate()
            .map(|(index, value)| format!(" {index}:{value:.0}"))
            .collect::<String>();
        format!("  cores[{values} ]")
    } else {
        String::new()
    };
    let cpu_label = if area.width >= 110 {
        format!(
            "CPU UTIL {}  {} cores{clusters}{per_core}",
            summary(snapshot.cpu.total_percent, &snapshot.cpu.history),
            snapshot.cpu.per_core_percent.len(),
        )
    } else if area.width >= 70 {
        format!(
            "CPU UTIL {}  {} cores{clusters}",
            summary(snapshot.cpu.total_percent, &snapshot.cpu.history),
            snapshot.cpu.per_core_percent.len(),
        )
    } else {
        format!(
            "CPU {}",
            short_summary(snapshot.cpu.total_percent, &snapshot.cpu.history)
        )
    };
    frame.render_widget(
        gauge(
            snapshot.cpu.total_percent,
            cpu_label,
            usage_color(Color::Cyan, snapshot.cpu.total_percent),
        ),
        rows[0],
    );
    let gpu = snapshot.gpu.utilization_percent;
    let renderer = optional_percent("render", snapshot.gpu.renderer_utilization_percent);
    let tiler = optional_percent("tiler", snapshot.gpu.tiler_utilization_percent);
    let gpu_name = if snapshot.gpu_info.name.is_empty() {
        "Apple GPU"
    } else {
        &snapshot.gpu_info.name
    };
    let gpu_label = match gpu {
        Some(value) if area.width >= 90 => format!(
            "GPU UTIL {}  {gpu_name}{renderer}{tiler}",
            summary(value, &snapshot.gpu.history)
        ),
        Some(value) if area.width >= 55 => {
            format!(
                "GPU UTIL {}  {gpu_name}",
                summary(value, &snapshot.gpu.history)
            )
        }
        Some(value) => format!("GPU {}", short_summary(value, &snapshot.gpu.history)),
        None => format!("GPU UTIL N/A  {gpu_name}"),
    };
    frame.render_widget(
        gauge(
            gpu.unwrap_or(0.0),
            gpu_label,
            usage_color(Color::Magenta, gpu.unwrap_or_default()),
        ),
        rows[1],
    );
    let mem_percent = snapshot.memory.used as f64 * 100.0 / snapshot.memory.total.max(1) as f64;
    let pressure = snapshot
        .memory
        .pressure_percent
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "N/A".into());
    let mem_label = if area.width >= 110 {
        format!(
            "UNIFIED MEM {}  {} / {}  avail {}  wired {}  cached {}  compressed {}  swap {} / {}  pressure {pressure}",
            summary(mem_percent, &snapshot.memory.history),
            bytes(snapshot.memory.used),
            bytes(snapshot.memory.total),
            bytes(snapshot.memory.available),
            bytes(snapshot.memory.wired),
            bytes(snapshot.memory.cached),
            bytes(snapshot.memory.compressed),
            bytes(snapshot.memory.swap_used),
            bytes(snapshot.memory.swap_total)
        )
    } else if area.width >= 70 {
        format!(
            "UNIFIED MEM {}  {} / {}  pressure {pressure}",
            summary(mem_percent, &snapshot.memory.history),
            bytes(snapshot.memory.used),
            bytes(snapshot.memory.total),
        )
    } else {
        format!(
            "MEM {}  {}/{}",
            short_summary(mem_percent, &snapshot.memory.history),
            bytes(snapshot.memory.used),
            bytes(snapshot.memory.total)
        )
    };
    frame.render_widget(
        gauge(
            mem_percent,
            mem_label,
            usage_color(Color::Green, mem_percent),
        ),
        rows[2],
    );
}

fn optional_percent(label: &str, value: Option<f64>) -> String {
    value
        .map(|value| format!("  {label} {value:.0}%"))
        .unwrap_or_default()
}

fn gauge(percent: f64, label: String, color: Color) -> Gauge<'static> {
    Gauge::default()
        .gauge_style(Style::default().fg(color))
        .ratio((percent / 100.0).clamp(0.0, 1.0))
        .label(label)
}

fn summary(current: f64, history: &History) -> String {
    let (average, peak) = history.summary(history.len()).unwrap_or((current, current));
    format!(
        "{:>3.0}%  avg {:>3.0}%  peak {:>3.0}%",
        current, average, peak
    )
}

fn short_summary(current: f64, history: &History) -> String {
    let (average, peak) = history.summary(history.len()).unwrap_or((current, current));
    format!("{current:.0}% avg{average:.0} peak{peak:.0}")
}

fn usage_color(normal: Color, percent: f64) -> Color {
    if percent >= 95.0 {
        Color::Red
    } else if percent >= 80.0 {
        Color::Yellow
    } else {
        normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_include_average_and_peak() {
        let mut history = History::new(3);
        history.push(10.0);
        history.push(30.0);
        assert_eq!(summary(30.0, &history), " 30%  avg  20%  peak  30%");
    }

    #[test]
    fn high_usage_changes_to_warning_colors() {
        assert_eq!(usage_color(Color::Cyan, 79.0), Color::Cyan);
        assert_eq!(usage_color(Color::Cyan, 80.0), Color::Yellow);
        assert_eq!(usage_color(Color::Cyan, 95.0), Color::Red);
    }
}
