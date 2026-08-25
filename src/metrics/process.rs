use crate::metrics::gpu_process::GpuProcessActivity;
use crate::model::{ProcessInfo, ProcessState, SortKey};
use crate::platform::macos::{libproc, sysctl};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
struct Previous {
    start_sec: u64,
    cpu_ns: u64,
}

pub struct ProcessCollector {
    previous: HashMap<i32, Previous>,
    sampled_at: Instant,
    argument_buffer_size: usize,
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self {
            previous: HashMap::new(),
            sampled_at: Instant::now(),
            argument_buffer_size: sysctl::value::<i32>("kern.argmax")
                .unwrap_or(262_144)
                .max(4096) as usize,
        }
    }
}

impl ProcessCollector {
    pub fn sample(
        &mut self,
        total_memory: u64,
        activities: &[GpuProcessActivity],
    ) -> Vec<ProcessInfo> {
        let now = Instant::now();
        let interval = now.duration_since(self.sampled_at).as_nanos().max(1) as f64;
        let wall_now = SystemTime::now();
        let mut next = HashMap::with_capacity(self.previous.len());
        let mut processes = Vec::with_capacity(activities.len());
        for activity in activities {
            let pid = activity.pid;
            let bsd = libproc::bsd_info(pid).ok();
            let task = libproc::task_info(pid).ok();
            let usage = libproc::rusage(pid).ok();
            let cpu_ns = usage
                .as_ref()
                .map(|u| u.user_time.saturating_add(u.system_time))
                .or_else(|| {
                    task.as_ref()
                        .map(|task| task.total_user.saturating_add(task.total_system))
                });
            let start_sec = bsd.as_ref().map(|bsd| bsd.start_sec).unwrap_or_default();
            let cpu_percent = cpu_ns
                .and_then(|cpu_ns| {
                    self.previous
                        .get(&pid)
                        .filter(|previous| previous.start_sec == start_sec)
                        .map(|previous| {
                            cpu_ns.saturating_sub(previous.cpu_ns) as f64 * 100.0 / interval
                        })
                })
                .unwrap_or(0.0);
            if let Some(cpu_ns) = cpu_ns {
                next.insert(pid, Previous { start_sec, cpu_ns });
            }
            let executable = libproc::executable(pid).unwrap_or_default();
            let registered_name = bsd
                .as_ref()
                .map(|bsd| sysctl::c_string(&bsd.name))
                .unwrap_or_default();
            let comm = bsd
                .as_ref()
                .map(|bsd| sysctl::c_string(&bsd.comm))
                .unwrap_or_default();
            let name = if !registered_name.is_empty() {
                registered_name
            } else if !comm.is_empty() {
                comm
            } else if !activity.name.is_empty() {
                activity.name.clone()
            } else {
                executable.rsplit('/').next().unwrap_or("?").to_owned()
            };
            let args =
                sysctl::process_arguments(pid, self.argument_buffer_size).unwrap_or_default();
            let command = if args.is_empty() {
                if executable.is_empty() {
                    name.clone()
                } else {
                    executable.clone()
                }
            } else {
                args.join(" ")
            };
            let memory_bytes = usage
                .as_ref()
                .map(|u| u.physical_footprint)
                .filter(|v| *v > 0)
                .or_else(|| task.as_ref().map(|task| task.resident_size))
                .unwrap_or_default();
            let start_time = bsd.as_ref().and_then(|bsd| {
                UNIX_EPOCH
                    .checked_add(Duration::from_secs(bsd.start_sec))
                    .and_then(|time| time.checked_add(Duration::from_micros(bsd.start_usec)))
            });
            processes.push(ProcessInfo {
                pid,
                ppid: bsd.as_ref().map(|bsd| bsd.ppid as i32).unwrap_or_default(),
                user: bsd
                    .as_ref()
                    .map(|bsd| libproc::username(bsd.uid))
                    .unwrap_or_default(),
                name,
                executable,
                command,
                gpu_time_ms_per_s: activity.gpu_time_ms_per_s,
                cpu_percent,
                memory_bytes,
                memory_percent: memory_bytes as f64 * 100.0 / total_memory.max(1) as f64,
                threads: task
                    .as_ref()
                    .map(|task| task.thread_count.max(0) as u32)
                    .unwrap_or_default(),
                state: bsd
                    .as_ref()
                    .map(|bsd| process_state(bsd.status))
                    .unwrap_or_default(),
                start_time,
                runtime: start_time
                    .and_then(|t| wall_now.duration_since(t).ok())
                    .unwrap_or_default(),
                cwd: libproc::current_directory(pid)
                    .ok()
                    .filter(|path| !path.is_empty()),
            });
        }
        self.previous = next;
        self.sampled_at = now;
        processes
    }
}

fn process_state(status: u32) -> ProcessState {
    match status {
        2 => ProcessState::Running,
        3 => ProcessState::Sleeping,
        4 => ProcessState::Stopped,
        5 => ProcessState::Zombie,
        _ => ProcessState::Unknown,
    }
}

pub fn visible<'a>(
    processes: &'a [ProcessInfo],
    filter: &str,
    key: SortKey,
) -> Vec<&'a ProcessInfo> {
    let needle = filter.to_lowercase();
    let mut items: Vec<_> = processes
        .iter()
        .filter(|p| {
            needle.is_empty()
                || p.name.to_lowercase().contains(&needle)
                || p.command.to_lowercase().contains(&needle)
                || p.pid.to_string().contains(&needle)
        })
        .collect();
    items.sort_unstable_by(|a, b| match key {
        SortKey::Gpu => b
            .gpu_time_ms_per_s
            .total_cmp(&a.gpu_time_ms_per_s)
            .then_with(|| a.pid.cmp(&b.pid)),
        SortKey::Pid => a.pid.cmp(&b.pid),
    });
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proc(pid: i32, name: &str, cpu: f64, mem: u64, gpu: f64) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.into(),
            command: name.into(),
            cpu_percent: cpu,
            memory_bytes: mem,
            gpu_time_ms_per_s: gpu,
            ..Default::default()
        }
    }
    #[test]
    fn filters_and_sorts_processes() {
        let input = vec![
            proc(2, "beta", 10.0, 20, 200.0),
            proc(1, "alpha", 20.0, 10, 100.0),
        ];
        assert_eq!(visible(&input, "alp", SortKey::Pid).len(), 1);
        assert_eq!(visible(&input, "", SortKey::Gpu)[0].pid, 2);
    }
}
