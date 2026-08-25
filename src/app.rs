use crate::demo::DemoRuntime;
use crate::event::{Event, EventLoop};
use crate::metrics::process::visible;
use crate::metrics::runtime::MetricsRuntime;
use crate::model::{Screen, Snapshot, SortKey, ViewState};
use crate::platform::macos::libproc;
use anyhow::Context;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::io::{self, IsTerminal};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

pub fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("mvitop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--startup-benchmark") {
        return startup_benchmark();
    }
    if !io::stdout().is_terminal() {
        anyhow::bail!(
            "mvitop needs an interactive terminal (try --startup-benchmark for a non-TUI check)"
        );
    }
    run_tui(args.iter().any(|arg| arg == "--demo"))
}

enum SnapshotSource {
    Live(MetricsRuntime),
    Demo(DemoRuntime),
}

impl SnapshotSource {
    fn snapshot(&mut self) -> Arc<Snapshot> {
        match self {
            Self::Live(runtime) => runtime.snapshot(),
            Self::Demo(runtime) => runtime.snapshot(),
        }
    }
}

fn run_tui(demo: bool) -> anyhow::Result<()> {
    let gpu_process_access = demo || authorize_gpu_processes();
    let started = Instant::now();
    install_panic_restore();
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("initialize terminal")?;
    terminal.clear().context("clear terminal")?;
    let mut view = ViewState::default();
    let empty = Snapshot::default();
    terminal
        .draw(|frame| crate::ui::render(frame, &empty, &view))
        .context("draw initial screen")?;
    let first_draw = started.elapsed();

    let mut source = if demo {
        SnapshotSource::Demo(DemoRuntime::new())
    } else {
        SnapshotSource::Live(MetricsRuntime::start(gpu_process_access))
    };
    let stopping = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, stopping.clone()).context("register SIGINT")?;
    signal_hook::flag::register(SIGTERM, stopping.clone()).context("register SIGTERM")?;
    let mut events = EventLoop::new(Duration::from_millis(crate::ui::refresh_millis(&view)));

    while !stopping.load(Ordering::Relaxed) {
        let snapshot = source.snapshot();
        normalize_selection(&mut view, &snapshot);
        terminal
            .draw(|frame| crate::ui::render(frame, &snapshot, &view))
            .context("render UI")?;
        match events.next().context("read terminal event")? {
            Event::Key(key) if handle_key(key, &mut view, &snapshot, terminal.size()?.height)? => {
                break;
            }
            Event::Key(_) | Event::Resize | Event::Tick => {}
        }
        events.set_interval(Duration::from_millis(crate::ui::refresh_millis(&view)));
    }
    drop(source);
    let _ = first_draw;
    Ok(())
}

fn authorize_gpu_processes() -> bool {
    if unsafe { libc::geteuid() } == 0 {
        return true;
    }
    if Command::new("/usr/bin/sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return true;
    }
    eprintln!(
        "mvitop needs administrator authorization to read per-process GPU time.\nOnly /usr/bin/powermetrics will run with elevated privileges."
    );
    Command::new("/usr/bin/sudo")
        .arg("-v")
        .status()
        .is_ok_and(|status| status.success())
}

fn handle_key(
    key: KeyEvent,
    view: &mut ViewState,
    snapshot: &Snapshot,
    height: u16,
) -> anyhow::Result<bool> {
    if key.kind == KeyEventKind::Release {
        return Ok(false);
    }
    if view.editing_filter {
        return Ok(handle_filter_key(key, view));
    }
    match view.screen {
        Screen::Help | Screen::Detail => {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?')
            ) {
                view.screen = Screen::Main;
            }
            return Ok(false);
        }
        Screen::Signal => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => view.screen = Screen::Main,
                KeyCode::Up | KeyCode::Char('k') => {
                    view.signal_index = view.signal_index.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    view.signal_index = (view.signal_index + 1).min(2)
                }
                KeyCode::Enter => {
                    view.message = Some(send_selected_signal(view));
                    view.screen = Screen::Main;
                    view.marked.clear();
                }
                _ => {}
            }
            return Ok(false);
        }
        Screen::Main => {}
    }
    match key.code {
        KeyCode::Char('q')
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::CONTROL =>
        {
            return Ok(true);
        }
        KeyCode::Char('?') | KeyCode::Char('h') => view.screen = Screen::Help,
        KeyCode::Down | KeyCode::Char('j') => move_selection(view, snapshot, 1),
        KeyCode::Up => move_selection(view, snapshot, -1),
        KeyCode::PageDown => move_selection(view, snapshot, (height / 2).max(1) as isize),
        KeyCode::PageUp => move_selection(view, snapshot, -((height / 2).max(1) as isize)),
        KeyCode::Char('c') => {
            view.sort_key = SortKey::Cpu;
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Char('m') => {
            view.sort_key = SortKey::Memory;
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Char('p') => {
            view.sort_key = SortKey::Pid;
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Char('g') => {
            view.sort_key = SortKey::Gpu;
            view.message = Some("per-process GPU data unavailable; rows remain PID ordered".into());
        }
        KeyCode::Char('/') => view.editing_filter = true,
        KeyCode::Char('t') => {
            view.tree = !view.tree;
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Enter => {
            if selected_process(view, snapshot).is_some() {
                view.screen = Screen::Detail;
            }
        }
        KeyCode::Char('k') => open_signal_menu(view, snapshot),
        KeyCode::Char(' ') => toggle_mark(view, snapshot),
        KeyCode::Char('r') => {
            view.refresh_rate_index = (view.refresh_rate_index + 1) % 4;
            view.message = Some(format!(
                "UI refresh: {} ms",
                crate::ui::refresh_millis(view)
            ));
        }
        _ => {}
    }
    Ok(false)
}

fn handle_filter_key(key: KeyEvent, view: &mut ViewState) -> bool {
    match key.code {
        KeyCode::Esc => {
            view.editing_filter = false;
        }
        KeyCode::Enter => {
            view.editing_filter = false;
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Backspace => {
            view.filter.pop();
            view.selected = 0;
            view.selected_pid = None;
        }
        KeyCode::Char(character)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            view.filter.push(character);
            view.selected = 0;
            view.selected_pid = None;
        }
        _ => {}
    }
    false
}

fn selected_process<'a>(
    view: &ViewState,
    snapshot: &'a Snapshot,
) -> Option<&'a crate::model::ProcessInfo> {
    if let Some(pid) = view.selected_pid {
        if let Some(process) = snapshot.processes.iter().find(|process| process.pid == pid) {
            return Some(process);
        }
    }
    visible(&snapshot.processes, &view.filter, view.sort_key, view.tree)
        .get(view.selected)
        .copied()
}

fn normalize_selection(view: &mut ViewState, snapshot: &Snapshot) {
    let items = visible(&snapshot.processes, &view.filter, view.sort_key, view.tree);
    if items.is_empty() {
        view.selected = 0;
        view.selected_pid = None;
        return;
    }
    if let Some(pid) = view.selected_pid {
        if let Some(index) = items.iter().position(|process| process.pid == pid) {
            view.selected = index;
            return;
        }
    }
    view.selected = view.selected.min(items.len() - 1);
    view.selected_pid = Some(items[view.selected].pid);
}

fn move_selection(view: &mut ViewState, snapshot: &Snapshot, delta: isize) {
    let items = visible(&snapshot.processes, &view.filter, view.sort_key, view.tree);
    if items.is_empty() {
        return;
    }
    let next = (view.selected as isize + delta).clamp(0, items.len() as isize - 1) as usize;
    view.selected = next;
    view.selected_pid = Some(items[next].pid);
    view.message = None;
}

fn toggle_mark(view: &mut ViewState, snapshot: &Snapshot) {
    let Some(process) = selected_process(view, snapshot) else {
        return;
    };
    if !view.marked.remove(&process.pid) {
        view.marked.insert(process.pid);
    }
}

fn open_signal_menu(view: &mut ViewState, snapshot: &Snapshot) {
    let mut targets: Vec<_> = if view.marked.is_empty() {
        selected_process(view, snapshot)
            .map(|p| vec![(p.pid, p.start_time)])
            .unwrap_or_default()
    } else {
        snapshot
            .processes
            .iter()
            .filter(|p| view.marked.contains(&p.pid))
            .map(|p| (p.pid, p.start_time))
            .collect()
    };
    targets.retain(|(pid, _)| *pid > 1 && *pid != std::process::id() as i32);
    if targets.is_empty() {
        view.message = Some("no safe signal target selected".into());
        return;
    }
    view.signal_targets = targets;
    view.signal_index = 0;
    view.screen = Screen::Signal;
}

fn send_selected_signal(view: &ViewState) -> String {
    let signal = [libc::SIGTERM, libc::SIGINT, libc::SIGKILL][view.signal_index.min(2)];
    let mut sent = 0;
    let mut skipped = 0;
    for (pid, expected_start) in &view.signal_targets {
        let current = libproc::bsd_info(*pid).ok();
        let same_process = current
            .as_ref()
            .zip(*expected_start)
            .and_then(|(bsd, start)| {
                start
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_secs() == bsd.start_sec)
            })
            .unwrap_or(false);
        if !same_process {
            skipped += 1;
            continue;
        }
        // SAFETY: PID identity was revalidated immediately above and signal is
        // one of three fixed constants, never user-provided shell input.
        if unsafe { libc::kill(*pid, signal) } == 0 {
            sent += 1;
        } else {
            skipped += 1;
        }
    }
    format!("signal {signal}: sent {sent}, skipped/failed {skipped}")
}

fn startup_benchmark() -> anyhow::Result<()> {
    let started = Instant::now();
    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| crate::ui::render(frame, &Snapshot::default(), &ViewState::default()))?;
    let first_frame = started.elapsed();
    let runtime_started = Instant::now();
    let runtime = MetricsRuntime::start(false);
    let runtime_ready = runtime_started.elapsed();
    let deadline = started + Duration::from_secs(5);
    let snapshot = loop {
        let snapshot = runtime.snapshot();
        if snapshot.memory.total > 0 && !snapshot.cpu.per_core_percent.is_empty() {
            break snapshot;
        }
        if Instant::now() >= deadline {
            break snapshot;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    println!(
        "first frame render: {:.3} ms",
        first_frame.as_secs_f64() * 1_000.0
    );
    println!(
        "runtime returned: {:.3} ms",
        runtime_ready.as_secs_f64() * 1_000.0
    );
    println!(
        "first CPU/memory snapshot: {:.3} ms",
        started.elapsed().as_secs_f64() * 1_000.0
    );
    println!(
        "CPU cores: {}, memory: {} bytes, GPU: {}",
        snapshot.cpu.per_core_percent.len(),
        snapshot.memory.total,
        snapshot
            .gpu
            .utilization_percent
            .map(|v| format!("{v:.0}%"))
            .unwrap_or_else(|| "N/A".into())
    );
    Ok(())
}

fn print_help() {
    println!(
        "mvitop {}\n\nUsage: mvitop [--demo] [--startup-benchmark]\n\nA native Apple Silicon system monitor. --demo uses synthetic data only.",
        env!("CARGO_PKG_VERSION")
    );
}

struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("enable terminal raw mode")?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
}

fn install_panic_restore() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_pid_reuse_without_matching_start_time() {
        let view = ViewState {
            signal_targets: vec![(i32::MAX, None)],
            ..ViewState::default()
        };
        assert!(send_selected_signal(&view).contains("skipped/failed 1"));
    }
}
