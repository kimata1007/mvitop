//! Minimal Mach host API FFI isolated from safe collectors.

use std::io;
use std::mem::{MaybeUninit, size_of};

const KERN_SUCCESS: i32 = 0;
const PROCESSOR_CPU_LOAD_INFO: i32 = 2;
const HOST_VM_INFO64: i32 = 4;

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct VmStatistics64 {
    pub free_count: u32,
    pub active_count: u32,
    pub inactive_count: u32,
    pub wire_count: u32,
    pub zero_fill_count: u64,
    pub reactivations: u64,
    pub pageins: u64,
    pub pageouts: u64,
    pub faults: u64,
    pub cow_faults: u64,
    pub lookups: u64,
    pub hits: u64,
    pub purges: u64,
    pub purgeable_count: u32,
    pub speculative_count: u32,
    pub decompressions: u64,
    pub compressions: u64,
    pub swapins: u64,
    pub swapouts: u64,
    pub compressor_page_count: u32,
    pub throttled_count: u32,
    pub external_page_count: u32,
    pub internal_page_count: u32,
    pub total_uncompressed_pages_in_compressor: u64,
    pub swapped_count: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuTicks {
    pub user: u64,
    pub system: u64,
    pub idle: u64,
    pub nice: u64,
}

unsafe extern "C" {
    fn mach_host_self() -> u32;
    static mach_task_self_: u32;
    fn host_statistics64(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn host_processor_info(
        host: u32,
        flavor: i32,
        processor_count: *mut u32,
        info: *mut *mut i32,
        info_count: *mut u32,
    ) -> i32;
    fn vm_deallocate(task: u32, address: usize, size: usize) -> i32;
}

pub fn vm_statistics() -> io::Result<VmStatistics64> {
    let mut stats = MaybeUninit::<VmStatistics64>::zeroed();
    let mut count = (size_of::<VmStatistics64>() / size_of::<i32>()) as u32;
    // SAFETY: stats is writable for count integers and has the SDK-specified layout.
    let result = unsafe {
        host_statistics64(
            mach_host_self(),
            HOST_VM_INFO64,
            stats.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if result != KERN_SUCCESS {
        return Err(io::Error::other(format!(
            "host_statistics64 failed: {result}"
        )));
    }
    // SAFETY: KERN_SUCCESS means the structure was initialized.
    Ok(unsafe { stats.assume_init() })
}

pub fn cpu_ticks() -> io::Result<Vec<CpuTicks>> {
    let mut cpu_count = 0u32;
    let mut info = std::ptr::null_mut::<i32>();
    let mut info_count = 0u32;
    // SAFETY: all output pointers are valid for writes.
    let result = unsafe {
        host_processor_info(
            mach_host_self(),
            PROCESSOR_CPU_LOAD_INFO,
            &mut cpu_count,
            &mut info,
            &mut info_count,
        )
    };
    if result != KERN_SUCCESS {
        return Err(io::Error::other(format!(
            "host_processor_info failed: {result}"
        )));
    }
    let expected = cpu_count as usize * 4;
    if info.is_null() || (info_count as usize) < expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short CPU tick buffer",
        ));
    }
    // SAFETY: the kernel returned at least expected integers.
    let raw = unsafe { std::slice::from_raw_parts(info.cast::<u32>(), expected) };
    let ticks = raw
        .chunks_exact(4)
        .map(|v| CpuTicks {
            user: v[0] as u64,
            system: v[1] as u64,
            idle: v[2] as u64,
            nice: v[3] as u64,
        })
        .collect();
    // SAFETY: host_processor_info allocates this buffer in our task map.
    let deallocated = unsafe {
        vm_deallocate(
            mach_task_self_,
            info as usize,
            info_count as usize * size_of::<i32>(),
        )
    };
    if deallocated != KERN_SUCCESS {
        return Err(io::Error::other(format!(
            "vm_deallocate failed: {deallocated}"
        )));
    }
    Ok(ticks)
}

pub fn page_size() -> u64 {
    // SAFETY: sysconf is side-effect free for this supported name.
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_host_counters() {
        assert!(!cpu_ticks().unwrap().is_empty());
        let vm = vm_statistics().unwrap();
        assert!(vm.free_count + vm.active_count + vm.inactive_count > 0);
    }
}
