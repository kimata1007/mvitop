use crate::metrics::runtime::{CPU_INTERVAL, GPU_INTERVAL, MEMORY_INTERVAL};
use crate::model::{History, Snapshot};
use crate::ui::LayoutMode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Sparkline};
use std::time::Duration;

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot, mode: LayoutMode) {
    if area.is_empty() {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1); 3])
        .split(area);
    render_one(
        frame,
        rows[0],
        "CPU",
        Some(snapshot.cpu.total_percent),
        &snapshot.cpu.history,
        Color::Cyan,
        CPU_INTERVAL,
        mode,
    );
    render_one(
        frame,
        rows[1],
        "GPU",
        snapshot.gpu.utilization_percent,
        &snapshot.gpu.history,
        Color::Magenta,
        GPU_INTERVAL,
        mode,
    );
    render_one(
        frame,
        rows[2],
        "MEM",
        Some(snapshot.memory.used as f64 * 100.0 / snapshot.memory.total.max(1) as f64),
        &snapshot.memory.history,
        Color::Green,
        MEMORY_INTERVAL,
        mode,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_one(
    frame: &mut Frame<'_>,
    area: Rect,
    name: &str,
    current: Option<f64>,
    history: &History,
    color: Color,
    sample_interval: Duration,
    mode: LayoutMode,
) {
    if area.is_empty() {
        return;
    }
    let title = history_title(name, current, history, mode);
    let history_duration = sample_interval.saturating_mul(history.len() as u32);
    let mut block = Block::default()
        .title(title)
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));
    if area.width >= 30 && area.height >= 3 {
        block = block
            .title_bottom(Line::from(format!(" -{} ", age(history_duration))).left_aligned())
            .title_bottom(Line::from(format!(" -{} ", age(history_duration / 2))).centered())
            .title_bottom(Line::from(" now ").right_aligned());
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let data = chart_data(history, inner.width as usize);
    frame.render_widget(
        Sparkline::default()
            .data(data)
            .max(100)
            .bar_set(symbols::bar::NINE_LEVELS)
            .style(Style::default().fg(color))
            .absent_value_symbol(" "),
        inner,
    );
}

fn history_title(name: &str, current: Option<f64>, history: &History, mode: LayoutMode) -> String {
    let current = current
        .map(|value| format!("{value:.0}%"))
        .unwrap_or_else(|| "N/A".into());
    if history.is_empty() {
        return format!(" {name} HISTORY  now {current}  waiting ");
    }
    let Some((average, peak)) = history.summary(history.len()) else {
        return format!(" {name} HISTORY  now {current}  waiting ");
    };
    if mode == LayoutMode::Full {
        format!(" {name} HISTORY  now {current}  avg {average:.0}%  peak {peak:.0}% ")
    } else {
        format!(" {name} {current}  avg {average:.0}%  peak {peak:.0}% ")
    }
}

fn chart_data(history: &History, width: usize) -> Vec<Option<u64>> {
    if width == 0 {
        return Vec::new();
    }
    let values = history.iter().copied().collect::<Vec<_>>();
    if values.len() <= width {
        let mut data = vec![None; width - values.len()];
        data.extend(values.into_iter().map(|value| Some(value.round() as u64)));
        return data;
    }

    (0..width)
        .map(|column| {
            let start = column * values.len() / width;
            let end = ((column + 1) * values.len() / width).max(start + 1);
            values[start..end]
                .iter()
                .copied()
                .reduce(f64::max)
                .map(|value| value.round() as u64)
        })
        .collect()
}

fn age(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds >= 60.0 {
        format!("{:.0}m", seconds / 60.0)
    } else if seconds >= 10.0 {
        format!("{seconds:.0}s")
    } else {
        format!("{seconds:.1}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_aligns_short_histories() {
        let mut history = History::new(4);
        history.push(10.0);
        history.push(20.0);
        assert_eq!(
            chart_data(&history, 4),
            vec![None, None, Some(10), Some(20)]
        );
    }

    #[test]
    fn downsampling_keeps_peaks() {
        let mut history = History::new(4);
        for value in [10.0, 90.0, 20.0, 30.0] {
            history.push(value);
        }
        assert_eq!(chart_data(&history, 2), vec![Some(90), Some(30)]);
    }

    #[test]
    fn formats_axis_ages_compactly() {
        assert_eq!(age(Duration::from_millis(3_500)), "3.5s");
        assert_eq!(age(Duration::from_secs(42)), "42s");
        assert_eq!(age(Duration::from_secs(120)), "2m");
    }
}
