use crate::metrics::cpu::CpuCollector;
use crate::metrics::gpu::{GpuBackend, UnavailableGpu};
use crate::metrics::gpu_process::PowermetricsSampler;
use crate::metrics::process::ProcessCollector;
use crate::metrics::{memory, system};
use crate::model::{History, Snapshot};
use crate::platform::macos::gpu::AppleGpuBackend;
use arc_swap::ArcSwap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

const CPU_INTERVAL: Duration = Duration::from_millis(500);
const MEMORY_INTERVAL: Duration = Duration::from_millis(500);
const GPU_INTERVAL: Duration = Duration::from_millis(350);
const SLOW_INTERVAL: Duration = Duration::from_secs(15);

type SharedSnapshot = Arc<ArcSwap<Snapshot>>;
type Shutdown = Arc<(Mutex<bool>, Condvar)>;

pub struct MetricsRuntime {
    snapshot: SharedSnapshot,
    shutdown: Shutdown,
    threads: Vec<JoinHandle<()>>,
}

impl MetricsRuntime {
    /// Returns immediately with an empty snapshot. Every potentially slow
    /// collector is initialized on its own worker after the first UI draw.
    pub fn start(gpu_process_access: bool) -> Self {
        let snapshot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));
        let threads = vec![
            spawn_system(snapshot.clone(), shutdown.clone()),
            spawn_cpu(snapshot.clone(), shutdown.clone()),
            spawn_memory(snapshot.clone(), shutdown.clone()),
            spawn_processes(snapshot.clone(), shutdown.clone(), gpu_process_access),
            spawn_gpu(snapshot.clone(), shutdown.clone()),
        ];
        Self {
            snapshot,
            shutdown,
            threads,
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.load_full()
    }
}

impl Drop for MetricsRuntime {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shutdown;
        if let Ok(mut stopped) = lock.lock() {
            *stopped = true;
        }
        wake.notify_all();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn update(shared: &SharedSnapshot, mut change: impl FnMut(&mut Snapshot)) {
    shared.rcu(|current| {
        let mut next = (**current).clone();
        change(&mut next);
        next.sequence = next.sequence.wrapping_add(1);
        next.captured_at = Some(SystemTime::now());
        next
    });
}

fn run_periodically(shutdown: &Shutdown, interval: Duration, mut sample: impl FnMut()) {
    loop {
        sample();
        let (lock, wake) = &**shutdown;
        let Ok(stopped) = lock.lock() else { break };
        if *stopped {
            break;
        }
        let Ok((stopped, _)) = wake.wait_timeout(stopped, interval) else {
            break;
        };
        if *stopped {
            break;
        }
    }
}

fn is_stopped(shutdown: &Shutdown) -> bool {
    shutdown.0.lock().map(|stopped| *stopped).unwrap_or(true)
}

fn wait_for_shutdown(shutdown: &Shutdown, interval: Duration) -> bool {
    let (lock, wake) = &**shutdown;
    let Ok(stopped) = lock.lock() else {
        return true;
    };
    if *stopped {
        return true;
    }
    wake.wait_timeout(stopped, interval)
        .map(|(stopped, _)| *stopped)
        .unwrap_or(true)
}

fn named_thread(name: &str, task: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("mvitop-{name}"))
        .spawn(task)
        .expect("failed to spawn metrics collector")
}

fn spawn_system(shared: SharedSnapshot, shutdown: Shutdown) -> JoinHandle<()> {
    named_thread("system", move || {
        run_periodically(&shutdown, SLOW_INTERVAL, || match system::collect() {
            Ok(info) => {
                let info = Arc::new(info);
                update(&shared, |snapshot| {
                    snapshot.system = info.clone();
                    snapshot.status.system_error = None;
                });
            }
            Err(error) => update(&shared, |snapshot| {
                snapshot.status.system_error = Some(error.to_string())
            }),
        })
    })
}

fn spawn_cpu(shared: SharedSnapshot, shutdown: Shutdown) -> JoinHandle<()> {
    named_thread("cpu", move || {
        let mut collector = CpuCollector::default();
        let mut history = History::default();
        run_periodically(&shutdown, CPU_INTERVAL, || {
            match collector.sample(&mut history) {
                Ok(sample) => update(&shared, |snapshot| {
                    snapshot.cpu = Arc::new(sample.clone());
                    snapshot.status.cpu_error = None;
                }),
                Err(error) => update(&shared, |snapshot| {
                    snapshot.status.cpu_error = Some(error.to_string())
                }),
            }
        });
    })
}

fn spawn_memory(shared: SharedSnapshot, shutdown: Shutdown) -> JoinHandle<()> {
    named_thread("memory", move || {
        let mut history = History::default();
        run_periodically(&shutdown, MEMORY_INTERVAL, || {
            match memory::collect(&mut history) {
                Ok(sample) => update(&shared, |snapshot| {
                    snapshot.memory = Arc::new(sample.clone());
                    snapshot.status.memory_error = None;
                }),
                Err(error) => update(&shared, |snapshot| {
                    snapshot.status.memory_error = Some(error.to_string())
                }),
            }
        });
    })
}

fn spawn_processes(
    shared: SharedSnapshot,
    shutdown: Shutdown,
    gpu_process_access: bool,
) -> JoinHandle<()> {
    named_thread("process", move || {
        if !gpu_process_access {
            update(&shared, |snapshot| {
                snapshot.status.process_error =
                    Some("GPU process monitoring needs administrator authorization".into())
            });
            return;
        }
        let mut collector = ProcessCollector::default();
        while !is_stopped(&shutdown) {
            match PowermetricsSampler::start() {
                Ok(mut sampler) => loop {
                    match sampler.next_sample() {
                        Ok(activities) => {
                            let total_memory = shared.load().memory.total;
                            let processes = Arc::new(collector.sample(total_memory, &activities));
                            update(&shared, |snapshot| {
                                snapshot.processes = processes.clone();
                                snapshot.status.process_error = None;
                            });
                        }
                        Err(error) => {
                            update(&shared, |snapshot| {
                                snapshot.status.process_error = Some(error.to_string())
                            });
                            break;
                        }
                    }
                    if is_stopped(&shutdown) {
                        break;
                    }
                },
                Err(error) => update(&shared, |snapshot| {
                    snapshot.status.process_error = Some(format!(
                        "cannot start privileged GPU process sampler: {error}"
                    ))
                }),
            }
            if wait_for_shutdown(&shutdown, Duration::from_secs(1)) {
                break;
            }
        }
    })
}

fn spawn_gpu(shared: SharedSnapshot, shutdown: Shutdown) -> JoinHandle<()> {
    named_thread("gpu", move || {
        let mut backend: Box<dyn GpuBackend> = match AppleGpuBackend::new() {
            Ok(gpu) => Box::new(gpu),
            Err(error) => Box::new(UnavailableGpu::new(error.to_string())),
        };
        match backend.static_info() {
            Ok(info) => update(&shared, |snapshot| {
                snapshot.gpu_info = Arc::new(info.clone())
            }),
            Err(error) => update(&shared, |snapshot| {
                snapshot.status.gpu_error = Some(error.to_string())
            }),
        }
        let mut history = History::default();
        run_periodically(&shutdown, GPU_INTERVAL, || match backend.sample() {
            Ok(mut sample) => {
                if let Some(value) = sample.utilization_percent {
                    history.push(value);
                }
                sample.history = history.clone();
                update(&shared, |snapshot| {
                    snapshot.gpu = Arc::new(sample.clone());
                    snapshot.status.gpu_error = None;
                });
            }
            Err(error) => update(&shared, |snapshot| {
                snapshot.status.gpu_error = Some(error.to_string())
            }),
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_publishes_without_waiting_for_all_collectors() {
        let runtime = MetricsRuntime::start(false);
        for _ in 0..100 {
            if runtime.snapshot().memory.total > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let snapshot = runtime.snapshot();
        assert!(snapshot.sequence > 0);
        assert!(snapshot.memory.total > 0);
    }
}
