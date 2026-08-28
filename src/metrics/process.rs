use crate::metrics::gpu_process::GpuProcessActivity;
use crate::model::{ProcessInfo, ProcessState, SortKey};
use crate::platform::macos::{libproc, sysctl};
use std::collections::{HashMap, HashSet};
use std::io;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CPU_ACTIVE_PERCENT: f64 = 0.1;
const RECENT_ACTIVITY: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
struct Previous {
    start_sec: u64,
    start_usec: u64,
    cpu_ns: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct JobKey {
    pid: i32,
    start_sec: u64,
    start_usec: u64,
}

#[derive(Clone, Debug)]
struct Node {
    pid: i32,
    ppid: i32,
    pgid: i32,
    tdev: u32,
    tpgid: i32,
    status: u32,
    start_sec: u64,
    start_usec: u64,
    name: String,
}

#[derive(Debug)]
struct JobDefinition {
    key: JobKey,
    root_pid: i32,
    members: Vec<i32>,
}

pub struct ProcessCollector {
    previous: HashMap<i32, Previous>,
    recent: HashMap<JobKey, Instant>,
    sampled_at: Instant,
    argument_buffer_size: usize,
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self {
            previous: HashMap::new(),
            recent: HashMap::new(),
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
    ) -> io::Result<Vec<ProcessInfo>> {
        let now = Instant::now();
        let interval = now.duration_since(self.sampled_at).as_nanos().max(1) as f64;
        let wall_now = SystemTime::now();
        let current_uid = unsafe { libc::geteuid() };
        let self_pid = std::process::id() as i32;
        let nodes = collect_nodes(current_uid)?;
        let jobs = define_jobs(&nodes, self_pid, |root| {
            let args =
                sysctl::process_arguments(root.pid, self.argument_buffer_size).unwrap_or_default();
            is_idle_shell(&root.name, &args)
        });
        let activities: HashMap<_, _> = activities
            .iter()
            .map(|activity| (activity.pid, activity))
            .collect();
        let mut next = HashMap::with_capacity(self.previous.len());
        let mut current_jobs = HashSet::with_capacity(jobs.len());
        let mut processes = Vec::with_capacity(jobs.len());
        let username = libproc::username(current_uid);

        for job in jobs {
            let Some(root) = nodes.get(&job.root_pid) else {
                continue;
            };
            current_jobs.insert(job.key);
            let mut cpu_percent = 0.0;
            let mut gpu_time_ms_per_s = 0.0;
            let mut memory_bytes = 0u64;
            let mut threads = 0u32;

            for pid in &job.members {
                let Some(node) = nodes.get(pid) else { continue };
                let task = libproc::task_info(*pid).ok();
                let usage = libproc::rusage(*pid).ok();
                let cpu_ns = usage
                    .as_ref()
                    .map(|usage| usage.user_time.saturating_add(usage.system_time))
                    .or_else(|| {
                        task.as_ref()
                            .map(|task| task.total_user.saturating_add(task.total_system))
                    });
                if let Some(cpu_ns) = cpu_ns {
                    if let Some(previous) = self.previous.get(pid).filter(|previous| {
                        previous.start_sec == node.start_sec
                            && previous.start_usec == node.start_usec
                    }) {
                        cpu_percent +=
                            cpu_ns.saturating_sub(previous.cpu_ns) as f64 * 100.0 / interval;
                    }
                    next.insert(
                        *pid,
                        Previous {
                            start_sec: node.start_sec,
                            start_usec: node.start_usec,
                            cpu_ns,
                        },
                    );
                }
                let member_memory = usage
                    .as_ref()
                    .map(|usage| usage.physical_footprint)
                    .filter(|value| *value > 0)
                    .or_else(|| task.as_ref().map(|task| task.resident_size))
                    .unwrap_or_default();
                memory_bytes = memory_bytes.saturating_add(member_memory);
                threads = threads.saturating_add(
                    task.as_ref()
                        .map(|task| task.thread_count.max(0) as u32)
                        .unwrap_or_default(),
                );
                gpu_time_ms_per_s += activities
                    .get(pid)
                    .map(|activity| activity.gpu_time_ms_per_s)
                    .unwrap_or_default();
            }

            let executable = libproc::executable(root.pid).unwrap_or_default();
            let args =
                sysctl::process_arguments(root.pid, self.argument_buffer_size).unwrap_or_default();
            let command = if args.is_empty() {
                if executable.is_empty() {
                    root.name.clone()
                } else {
                    executable.clone()
                }
            } else {
                args.join(" ")
            };
            let start_time = UNIX_EPOCH
                .checked_add(Duration::from_secs(root.start_sec))
                .and_then(|time| time.checked_add(Duration::from_micros(root.start_usec)));
            let process = ProcessInfo {
                pid: root.pid,
                ppid: root.ppid,
                member_count: job.members.len() as u32,
                user: username.clone(),
                name: root.name.clone(),
                executable,
                command,
                gpu_time_ms_per_s,
                cpu_percent,
                memory_bytes,
                memory_percent: memory_bytes as f64 * 100.0 / total_memory.max(1) as f64,
                threads,
                state: process_state(root.status),
                start_time,
                runtime: start_time
                    .and_then(|t| wall_now.duration_since(t).ok())
                    .unwrap_or_default(),
                cwd: libproc::current_directory(root.pid)
                    .ok()
                    .filter(|path| !path.is_empty()),
            };
            let active =
                process.cpu_percent >= CPU_ACTIVE_PERCENT || process.gpu_time_ms_per_s > 0.0;
            if active {
                self.recent.insert(job.key, now);
            }
            if active
                || self
                    .recent
                    .get(&job.key)
                    .is_some_and(|last| now.saturating_duration_since(*last) <= RECENT_ACTIVITY)
            {
                processes.push(process);
            }
        }
        self.previous = next;
        self.recent.retain(|key, last| {
            current_jobs.contains(key) && now.saturating_duration_since(*last) <= RECENT_ACTIVITY
        });
        self.sampled_at = now;
        Ok(processes)
    }
}

fn collect_nodes(uid: u32) -> io::Result<HashMap<i32, Node>> {
    let mut nodes = HashMap::new();
    for pid in libproc::list_pids()? {
        let Ok(bsd) = libproc::bsd_info(pid) else {
            continue;
        };
        if bsd.uid != uid {
            continue;
        }
        let registered_name = sysctl::c_string(&bsd.name);
        let comm = sysctl::c_string(&bsd.comm);
        nodes.insert(
            pid,
            Node {
                pid,
                ppid: bsd.ppid as i32,
                pgid: bsd.pgid as i32,
                tdev: bsd.tdev,
                tpgid: bsd.tpgid as i32,
                status: bsd.status,
                start_sec: bsd.start_sec,
                start_usec: bsd.start_usec,
                name: if registered_name.is_empty() {
                    comm
                } else {
                    registered_name
                },
            },
        );
    }
    Ok(nodes)
}

fn define_jobs(
    nodes: &HashMap<i32, Node>,
    self_pid: i32,
    mut is_idle: impl FnMut(&Node) -> bool,
) -> Vec<JobDefinition> {
    let mut groups: HashMap<(u32, i32), Vec<i32>> = HashMap::new();
    for node in nodes.values() {
        let has_terminal = node.tdev != 0 && node.tdev != u32::MAX && node.tpgid > 0;
        if has_terminal && node.pgid == node.tpgid {
            groups
                .entry((node.tdev, node.tpgid))
                .or_default()
                .push(node.pid);
        }
    }

    let mut definitions = Vec::new();
    let mut direct_membership = HashMap::new();
    for members in groups.into_values() {
        if members.contains(&self_pid) {
            continue;
        }
        let member_set: HashSet<_> = members.iter().copied().collect();
        let root_pid = members
            .iter()
            .copied()
            .filter(|pid| {
                nodes
                    .get(pid)
                    .is_some_and(|node| !member_set.contains(&node.ppid))
            })
            .min_by_key(|pid| {
                nodes
                    .get(pid)
                    .map(|node| (node.start_sec, node.start_usec, node.pid))
                    .unwrap_or((u64::MAX, u64::MAX, i32::MAX))
            })
            .or_else(|| members.iter().copied().min());
        let Some(root_pid) = root_pid else { continue };
        let Some(root) = nodes.get(&root_pid) else {
            continue;
        };
        if is_idle(root) || matches!(root.name.as_str(), "sudo" | "doas" | "login") {
            continue;
        }
        let index = definitions.len();
        for pid in &members {
            direct_membership.insert(*pid, index);
        }
        definitions.push(JobDefinition {
            key: JobKey {
                pid: root.pid,
                start_sec: root.start_sec,
                start_usec: root.start_usec,
            },
            root_pid,
            members: Vec::new(),
        });
    }

    for pid in nodes.keys().copied() {
        let mut cursor = pid;
        let mut traversed = 0usize;
        let job_index = loop {
            if let Some(index) = direct_membership.get(&cursor) {
                break Some(*index);
            }
            let Some(parent) = nodes.get(&cursor).map(|node| node.ppid) else {
                break None;
            };
            if parent <= 1 || parent == cursor || traversed >= nodes.len() {
                break None;
            }
            cursor = parent;
            traversed += 1;
        };
        if let Some(index) = job_index {
            definitions[index].members.push(pid);
        }
    }
    for definition in &mut definitions {
        definition.members.sort_unstable();
    }
    definitions.sort_unstable_by_key(|job| job.root_pid);
    definitions
}

fn is_idle_shell(name: &str, args: &[String]) -> bool {
    let name = name.trim_start_matches('-');
    let is_shell = matches!(
        name,
        "sh" | "bash" | "dash" | "fish" | "ksh" | "nu" | "tcsh" | "xonsh" | "zsh"
    );
    if !is_shell {
        return false;
    }
    !args
        .iter()
        .skip(1)
        .any(|arg| arg == "-c" || arg == "--command" || (!arg.starts_with('-') && !arg.is_empty()))
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
            .then_with(|| b.cpu_percent.total_cmp(&a.cpu_percent))
            .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
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

    fn node(pid: i32, ppid: i32, pgid: i32, tpgid: i32, name: &str) -> Node {
        Node {
            pid,
            ppid,
            pgid,
            tdev: 1,
            tpgid,
            status: 2,
            start_sec: pid as u64,
            start_usec: 0,
            name: name.into(),
        }
    }

    #[test]
    fn finds_foreground_job_and_aggregates_descendants() {
        let nodes = HashMap::from([
            (10, node(10, 1, 10, 20, "zsh")),
            (20, node(20, 10, 20, 20, "cargo")),
            (21, node(21, 20, 21, 20, "rustc")),
            (30, node(30, 1, 30, 30, "mvitop")),
            (40, node(40, 1, 40, 0, "Code Helper")),
        ]);
        let jobs = define_jobs(&nodes, 30, |_| false);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].root_pid, 20);
        assert_eq!(jobs[0].members, vec![20, 21]);
    }

    #[test]
    fn recognizes_idle_and_script_shells() {
        assert!(is_idle_shell("zsh", &["/bin/zsh".into(), "-l".into()]));
        assert!(!is_idle_shell(
            "bash",
            &["/bin/bash".into(), "build.sh".into()]
        ));
        assert!(!is_idle_shell(
            "zsh",
            &["/bin/zsh".into(), "-c".into(), "make".into()]
        ));
    }
}
