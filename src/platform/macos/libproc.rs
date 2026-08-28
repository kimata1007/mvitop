//! Minimal libproc FFI isolated from safe collectors.

use crate::platform::macos::sysctl;
use std::ffi::CStr;
use std::io;
use std::mem::{MaybeUninit, size_of};

const PROC_PIDTBSDINFO: i32 = 3;
const PROC_PIDTASKINFO: i32 = 4;
const PROC_PIDVNODEPATHINFO: i32 = 9;
const PROC_ALL_PIDS: u32 = 1;
const RUSAGE_INFO_V4: i32 = 4;
const PROC_PIDPATHINFO_MAXSIZE: usize = 4_096;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BsdInfo {
    pub flags: u32,
    pub status: u32,
    pub xstatus: u32,
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub gid: u32,
    pub ruid: u32,
    pub rgid: u32,
    pub svuid: u32,
    pub svgid: u32,
    pub reserved: u32,
    pub comm: [libc::c_char; 16],
    pub name: [libc::c_char; 32],
    pub nfiles: u32,
    pub pgid: u32,
    pub pjobc: u32,
    pub tdev: u32,
    pub tpgid: u32,
    pub nice: i32,
    pub start_sec: u64,
    pub start_usec: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub total_user: u64,
    pub total_system: u64,
    pub threads_user: u64,
    pub threads_system: u64,
    pub policy: i32,
    pub faults: i32,
    pub pageins: i32,
    pub cow_faults: i32,
    pub messages_sent: i32,
    pub messages_received: i32,
    pub syscalls_mach: i32,
    pub syscalls_unix: i32,
    pub context_switches: i32,
    pub thread_count: i32,
    pub running_threads: i32,
    pub priority: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RusageInfoV4 {
    pub uuid: [u8; 16],
    pub user_time: u64,
    pub system_time: u64,
    pub package_idle_wakeups: u64,
    pub interrupt_wakeups: u64,
    pub pageins: u64,
    pub wired_size: u64,
    pub resident_size: u64,
    pub physical_footprint: u64,
    pub process_start_absolute_time: u64,
    pub process_exit_absolute_time: u64,
    pub child_user_time: u64,
    pub child_system_time: u64,
    pub child_package_idle_wakeups: u64,
    pub child_interrupt_wakeups: u64,
    pub child_pageins: u64,
    pub child_elapsed_absolute_time: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,
    pub cpu_time_qos_default: u64,
    pub cpu_time_qos_maintenance: u64,
    pub cpu_time_qos_background: u64,
    pub cpu_time_qos_utility: u64,
    pub cpu_time_qos_legacy: u64,
    pub cpu_time_qos_user_initiated: u64,
    pub cpu_time_qos_user_interactive: u64,
    pub billed_system_time: u64,
    pub serviced_system_time: u64,
    pub logical_writes: u64,
    pub lifetime_max_physical_footprint: u64,
    pub instructions: u64,
    pub cycles: u64,
    pub billed_energy: u64,
    pub serviced_energy: u64,
    pub interval_max_physical_footprint: u64,
    pub runnable_time: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VnodeInfoPath {
    vnode_info: [u8; 152],
    path: [libc::c_char; 1024],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VnodePathInfo {
    current_directory: VnodeInfoPath,
    root_directory: VnodeInfoPath,
}

unsafe extern "C" {
    fn proc_listpids(
        process_type: u32,
        type_info: u32,
        buffer: *mut libc::c_void,
        buffer_size: i32,
    ) -> i32;
    fn proc_pidinfo(
        pid: i32,
        flavor: i32,
        arg: u64,
        buffer: *mut libc::c_void,
        buffer_size: i32,
    ) -> i32;
    fn proc_pidpath(pid: i32, buffer: *mut libc::c_void, buffer_size: u32) -> i32;
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RusageInfoV4) -> i32;
}

pub fn list_pids() -> io::Result<Vec<i32>> {
    // SAFETY: a null buffer asks libproc for the required byte count.
    let required = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if required <= 0 {
        return Err(io::Error::last_os_error());
    }

    // Leave spare entries for processes created between the sizing and data calls.
    let mut pids = vec![0i32; required as usize / size_of::<i32>() + 32];
    let buffer_size = i32::try_from(pids.len() * size_of::<i32>()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "process list buffer is too large",
        )
    })?;
    // SAFETY: pids owns buffer_size writable bytes for the duration of the call.
    let read = unsafe { proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), buffer_size) };
    if read <= 0 {
        return Err(io::Error::last_os_error());
    }
    pids.truncate(read as usize / size_of::<i32>());
    pids.retain(|pid| *pid > 0);
    Ok(pids)
}

fn pid_info<T: Copy>(pid: i32, flavor: i32) -> io::Result<T> {
    let mut value = MaybeUninit::<T>::uninit();
    // SAFETY: value is aligned and writable for the requested SDK structure.
    let read = unsafe {
        proc_pidinfo(
            pid,
            flavor,
            0,
            value.as_mut_ptr().cast(),
            size_of::<T>() as i32,
        )
    };
    if read != size_of::<T>() as i32 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a complete structure was returned.
    Ok(unsafe { value.assume_init() })
}

pub fn bsd_info(pid: i32) -> io::Result<BsdInfo> {
    pid_info(pid, PROC_PIDTBSDINFO)
}
pub fn task_info(pid: i32) -> io::Result<TaskInfo> {
    pid_info(pid, PROC_PIDTASKINFO)
}

pub fn rusage(pid: i32) -> io::Result<RusageInfoV4> {
    let mut usage = RusageInfoV4::default();
    // SAFETY: usage has the exact RUSAGE_INFO_V4 layout.
    if unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V4, &mut usage) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(usage)
}

pub fn executable(pid: i32) -> io::Result<String> {
    let mut path = vec![0i8; PROC_PIDPATHINFO_MAXSIZE];
    // SAFETY: path is a writable buffer with the supplied length.
    let read = unsafe { proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    if read <= 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: proc_pidpath NUL-terminates a successful result.
    Ok(unsafe { CStr::from_ptr(path.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

pub fn current_directory(pid: i32) -> io::Result<String> {
    let info: VnodePathInfo = pid_info(pid, PROC_PIDVNODEPATHINFO)?;
    Ok(sysctl::c_string(&info.current_directory.path))
}

pub fn username(uid: u32) -> String {
    let mut pwd = MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 1024];
    // SAFETY: all buffers meet getpwuid_r's contract and remain live for the copy.
    let code = unsafe {
        libc::getpwuid_r(
            uid,
            pwd.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code == 0 && !result.is_null() {
        // SAFETY: successful getpwuid_r initializes pwd and pw_name.
        return unsafe { CStr::from_ptr((*pwd.as_ptr()).pw_name) }
            .to_string_lossy()
            .into_owned();
    }
    uid.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_current_process() {
        let pid = std::process::id() as i32;
        assert!(list_pids().unwrap().contains(&pid));
        assert_eq!(bsd_info(pid).unwrap().pid, pid as u32);
        assert!(task_info(pid).unwrap().thread_count > 0);
        assert!(!executable(pid).unwrap().is_empty());
        assert!(!current_directory(pid).unwrap().is_empty());
    }
}
