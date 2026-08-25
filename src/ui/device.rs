use crate::model::Snapshot;
use crate::ui::bytes;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Gauge};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1); 3])
        .margin(1)
        .split(area);
    let gpu = snapshot.gpu.utilization_percent;
    let gpu_label = format!(
        "GPU {:>5}  {:<18}  freq {}  power {}  temp {}",
        gpu.map(|v| format!("{v:.0}%"))
            .unwrap_or_else(|| "N/A".into()),
        if snapshot.gpu_info.name.is_empty() {
            "Apple GPU"
        } else {
            &snapshot.gpu_info.name
        },
        snapshot
            .gpu
            .frequency_hz
            .map(|v| format!("{:.2}GHz", v as f64 / 1e9))
            .unwrap_or_else(|| "N/A".into()),
        snapshot
            .gpu
            .power_watts
            .map(|v| format!("{v:.1}W"))
            .unwrap_or_else(|| "N/A".into()),
        snapshot
            .gpu
            .temperature_celsius
            .map(|v| format!("{v:.0}°C"))
            .unwrap_or_else(|| "N/A".into())
    );
    frame.render_widget(
        gauge(gpu.unwrap_or(0.0), gpu_label, Color::Magenta),
        rows[0],
    );
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
    frame.render_widget(
        gauge(
            snapshot.cpu.total_percent,
            format!(
                "CPU {:>4.0}%  {} cores{clusters}{per_core}",
                snapshot.cpu.total_percent,
                snapshot.cpu.per_core_percent.len()
            ),
            Color::Cyan,
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
            "MEM {:>4.0}%  {} / {}  avail {}  wired {}  cached {}  compressed {}  swap {} / {}  pressure {pressure}",
            mem_percent,
            bytes(snapshot.memory.used),
            bytes(snapshot.memory.total),
            bytes(snapshot.memory.available),
            bytes(snapshot.memory.wired),
            bytes(snapshot.memory.cached),
            bytes(snapshot.memory.compressed),
            bytes(snapshot.memory.swap_used),
            bytes(snapshot.memory.swap_total)
        )
    } else {
        format!(
            "MEM {:>4.0}%  {} / {}  compressed {}  swap {} / {}  pressure {pressure}",
            mem_percent,
            bytes(snapshot.memory.used),
            bytes(snapshot.memory.total),
            bytes(snapshot.memory.compressed),
            bytes(snapshot.memory.swap_used),
            bytes(snapshot.memory.swap_total)
        )
    };
    frame.render_widget(gauge(mem_percent, mem_label, Color::Green), rows[2]);
    frame.render_widget(
        Block::default().borders(Borders::LEFT | Borders::RIGHT),
        area,
    );
}

fn gauge(percent: f64, label: String, color: Color) -> Gauge<'static> {
    Gauge::default()
        .gauge_style(Style::default().fg(color))
        .ratio((percent / 100.0).clamp(0.0, 1.0))
        .label(label)
}
