use crate::metrics::process::{tree_depth, visible};
use crate::model::{Snapshot, SortKey, ViewState};
use crate::ui::{bytes, duration};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Row, Table, TableState};

pub fn render(frame: &mut Frame<'_>, area: Rect, snapshot: &Snapshot, view: &ViewState) {
    let items = visible(&snapshot.processes, &view.filter, view.sort_key, view.tree);
    let wide = area.width >= 100;
    let medium = area.width >= 72;
    let mut header = vec![" ", "PID", "CPU%", "MEM"];
    let mut widths = vec![
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(9),
    ];
    if medium {
        header.extend(["GPU", "USER"]);
        widths.extend([Constraint::Length(5), Constraint::Length(10)]);
    }
    if wide {
        header.extend(["THR", "S", "TIME"]);
        widths.extend([
            Constraint::Length(5),
            Constraint::Length(2),
            Constraint::Length(9),
        ]);
    }
    header.push("COMMAND");
    widths.push(Constraint::Min(12));
    let rows = items.iter().map(|process| {
        let mut values = vec![
            if view.marked.contains(&process.pid) {
                "*".to_owned()
            } else {
                " ".to_owned()
            },
            process.pid.to_string(),
            format!("{:.1}", process.cpu_percent),
            bytes(process.memory_bytes),
        ];
        if medium {
            values.extend(["--".to_owned(), process.user.clone()]);
        }
        if wide {
            values.extend([
                process.threads.to_string(),
                process.state.short().to_string(),
                duration(process.runtime),
            ]);
        }
        let indent = if view.tree {
            "  ".repeat(tree_depth(process, &snapshot.processes))
        } else {
            String::new()
        };
        values.push(format!("{indent}{}", process.command));
        Row::new(values).style(if view.marked.contains(&process.pid) {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        })
    });
    let sort = match view.sort_key {
        SortKey::Cpu => "CPU",
        SortKey::Memory => "MEM",
        SortKey::Gpu => "GPU",
        SortKey::Pid => "PID",
    };
    let title = format!(
        " Processes {}  {} shown  sort:{sort}{} ",
        snapshot.processes.len(),
        items.len(),
        if view.tree { "  tree" } else { "" }
    );
    let table = Table::new(rows, widths)
        .header(
            Row::new(header.into_iter().map(Cell::from)).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    let mut state = TableState::default().with_selected(if items.is_empty() {
        None
    } else {
        Some(view.selected.min(items.len() - 1))
    });
    frame.render_stateful_widget(table, area, &mut state);
}
