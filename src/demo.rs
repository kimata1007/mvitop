//! Deterministic, synthetic data used for documentation recordings.
//!
//! This mode never initializes a collector and therefore cannot expose the
//! machine model, username, process list, paths, or other host information.

use crate::model::{
    CpuSample, GpuInfo, GpuSample, History, MemorySample, ProcessInfo, ProcessState, Snapshot,
    SystemInfo,
};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

const GIB: u64 = 1024 * 1024 * 1024;

pub struct DemoRuntime {
    started: Instant,
    sequence: u64,
    cpu_history: History,
    gpu_history: History,
    memory_history: History,
}

impl DemoRuntime {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            sequence: 0,
            cpu_history: synthetic_history(30.0, 16.0, 1.25),
            gpu_history: synthetic_history(64.0, 22.0, 0.85),
            memory_history: synthetic_history(42.0, 3.0, 0.3),
        }
    }

    pub fn snapshot(&mut self) -> Arc<Snapshot> {
        let phase = self.started.elapsed().as_secs_f64();
        let cpu = wave(phase, 30.0, 16.0, 1.25);
        let gpu = wave(phase, 64.0, 22.0, 0.85);
        let memory_percent = wave(phase, 42.0, 3.0, 0.3);
        self.cpu_history.push(cpu);
        self.gpu_history.push(gpu);
        self.memory_history.push(memory_percent);
        self.sequence = self.sequence.wrapping_add(1);

        let total = 64 * GIB;
        let used = (total as f64 * memory_percent / 100.0) as u64;
        let processes = synthetic_processes(phase, total);
        Arc::new(Snapshot {
            sequence: self.sequence,
            captured_at: Some(SystemTime::now()),
            system: Arc::new(SystemInfo {
                model: "Demo Mac".into(),
                soc: "Apple M4 Max · synthetic data".into(),
                os_version: "26.1".into(),
                performance_cores: Some(12),
                efficiency_cores: Some(4),
                uptime: Duration::from_secs(3 * 86_400 + 7 * 3_600 + phase as u64),
                load_average: [2.18, 2.04, 1.92],
            }),
            cpu: Arc::new(CpuSample {
                total_percent: cpu,
                per_core_percent: (0..16)
                    .map(|core| wave(phase + core as f64 * 0.19, 28.0, 19.0, 1.1))
                    .collect(),
                history: self.cpu_history.clone(),
            }),
            memory: Arc::new(MemorySample {
                total,
                used,
                available: total - used,
                wired: 5 * GIB,
                compressed: 3 * GIB,
                cached: 14 * GIB,
                swap_used: GIB / 2,
                swap_total: 4 * GIB,
                pressure_percent: None,
                history: self.memory_history.clone(),
            }),
            gpu_info: Arc::new(GpuInfo {
                name: "Apple M4 Max".into(),
            }),
            gpu: Arc::new(GpuSample {
                utilization_percent: Some(gpu),
                renderer_utilization_percent: Some(wave(phase, 58.0, 20.0, 0.9)),
                tiler_utilization_percent: Some(wave(phase, 46.0, 18.0, 0.8)),
                history: self.gpu_history.clone(),
            }),
            processes: Arc::new(processes),
            ..Snapshot::default()
        })
    }
}

fn synthetic_history(center: f64, amplitude: f64, speed: f64) -> History {
    let mut history = History::default();
    for sample in 0..history.capacity() {
        let phase = (sample as f64 + 1.0 - history.capacity() as f64) * 0.5;
        history.push(wave(phase, center, amplitude, speed));
    }
    history
}

fn wave(phase: f64, center: f64, amplitude: f64, speed: f64) -> f64 {
    (center + amplitude * (phase * speed).sin()).clamp(0.0, 100.0)
}

fn synthetic_processes(phase: f64, total_memory: u64) -> Vec<ProcessInfo> {
    let rows = [
        (
            4210,
            1,
            "render-worker",
            "python render_scene.py --device metal",
            13 * GIB,
            164,
            ProcessState::Running,
            487.0,
        ),
        (
            3172,
            1,
            "model-server",
            "model-server --listen 127.0.0.1",
            7 * GIB,
            58,
            ProcessState::Sleeping,
            102.0,
        ),
        (
            6110,
            4100,
            "video-encoder",
            "video-encoder demo.mov output.mp4",
            GIB,
            24,
            ProcessState::Running,
            39.0,
        ),
        (
            8124,
            3172,
            "inference",
            "inference --model demo-model",
            5 * GIB,
            42,
            ProcessState::Sleeping,
            12.0,
        ),
    ];
    rows.into_iter()
        .enumerate()
        .map(
            |(index, (pid, ppid, name, command, memory, threads, state, gpu_time))| {
                let cpu = wave(
                    phase + index as f64 * 0.63,
                    18.0 + index as f64 * 2.0,
                    14.0,
                    1.0,
                );
                ProcessInfo {
                    pid,
                    ppid,
                    member_count: (index as u32 + 1) * 2,
                    user: "demo".into(),
                    name: name.into(),
                    executable: format!("/opt/demo/bin/{name}"),
                    command: command.into(),
                    gpu_time_ms_per_s: (gpu_time
                        + 18.0 * (phase * 0.9 + index as f64 * 0.41).sin())
                    .clamp(0.1, 1_000.0),
                    cpu_percent: cpu,
                    memory_bytes: memory,
                    memory_percent: memory as f64 * 100.0 / total_memory as f64,
                    threads,
                    state,
                    start_time: None,
                    runtime: Duration::from_secs(600 + index as u64 * 317 + phase as u64),
                    cwd: Some("/tmp/mvitop-demo".into()),
                }
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_contains_only_synthetic_identifiers() {
        let mut runtime = DemoRuntime::new();
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.system.model, "Demo Mac");
        assert_eq!(snapshot.cpu.history.len(), snapshot.cpu.history.capacity());
        assert!(
            snapshot
                .processes
                .iter()
                .all(|process| process.user == "demo")
        );
        assert!(
            snapshot
                .processes
                .iter()
                .all(|process| !process.command.contains("/Users/"))
        );
    }
}
