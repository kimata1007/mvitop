use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, Default)]
pub struct SystemInfo {
    pub model: String,
    pub soc: String,
    pub os_version: String,
    pub performance_cores: Option<u32>,
    pub efficiency_cores: Option<u32>,
    pub uptime: Duration,
    pub load_average: [f64; 3],
}

#[derive(Clone, Debug, Default)]
pub struct CpuSample {
    pub total_percent: f64,
    pub per_core_percent: Vec<f64>,
    pub history: History,
}

#[derive(Clone, Debug, Default)]
pub struct MemorySample {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub wired: u64,
    pub compressed: u64,
    pub cached: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub pressure_percent: Option<f64>,
    pub history: History,
}

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct GpuSample {
    pub utilization_percent: Option<f64>,
    pub frequency_hz: Option<u64>,
    pub power_watts: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub history: History,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProcessState {
    Running,
    Sleeping,
    Stopped,
    Zombie,
    #[default]
    Unknown,
}

impl ProcessState {
    pub fn short(self) -> char {
        match self {
            Self::Running => 'R',
            Self::Sleeping => 'S',
            Self::Stopped => 'T',
            Self::Zombie => 'Z',
            Self::Unknown => '?',
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcessInfo {
    pub pid: i32,
    pub ppid: i32,
    pub user: String,
    pub name: String,
    pub executable: String,
    pub command: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_percent: f64,
    pub threads: u32,
    pub state: ProcessState,
    pub start_time: Option<SystemTime>,
    pub runtime: Duration,
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct MetricStatus {
    pub system_error: Option<String>,
    pub cpu_error: Option<String>,
    pub memory_error: Option<String>,
    pub process_error: Option<String>,
    pub gpu_error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub sequence: u64,
    pub captured_at: Option<SystemTime>,
    pub system: Arc<SystemInfo>,
    pub cpu: Arc<CpuSample>,
    pub memory: Arc<MemorySample>,
    pub gpu_info: Arc<GpuInfo>,
    pub gpu: Arc<GpuSample>,
    pub processes: Arc<Vec<ProcessInfo>>,
    pub status: MetricStatus,
}

#[derive(Clone, Debug)]
pub struct History {
    values: VecDeque<f64>,
    capacity: usize,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
    pub fn push(&mut self, value: f64) {
        if self.capacity == 0 {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value.clamp(0.0, 100.0));
    }
    pub fn iter(&self) -> impl Iterator<Item = &f64> {
        self.values.iter()
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(120)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortKey {
    #[default]
    Cpu,
    Memory,
    Gpu,
    Pid,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    #[default]
    Main,
    Help,
    Detail,
    Signal,
}

#[derive(Debug)]
pub struct ViewState {
    pub screen: Screen,
    pub selected: usize,
    pub selected_pid: Option<i32>,
    pub sort_key: SortKey,
    pub filter: String,
    pub editing_filter: bool,
    pub tree: bool,
    pub marked: HashSet<i32>,
    pub signal_index: usize,
    pub signal_targets: Vec<(i32, Option<SystemTime>)>,
    pub refresh_rate_index: usize,
    pub message: Option<String>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            screen: Screen::Main,
            selected: 0,
            selected_pid: None,
            sort_key: SortKey::Cpu,
            filter: String::new(),
            editing_filter: false,
            tree: false,
            marked: HashSet::new(),
            signal_index: 0,
            signal_targets: Vec::new(),
            refresh_rate_index: 1,
            message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_a_bounded_ring() {
        let mut history = History::new(3);
        for value in [1.0, 2.0, 3.0, 4.0] {
            history.push(value);
        }
        assert_eq!(
            history.iter().copied().collect::<Vec<_>>(),
            vec![2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn history_clamps_percentages() {
        let mut history = History::new(2);
        history.push(-1.0);
        history.push(101.0);
        assert_eq!(
            history.iter().copied().collect::<Vec<_>>(),
            vec![0.0, 100.0]
        );
    }
}
