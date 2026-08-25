use crate::model::{History, MemorySample};
use crate::platform::macos::{mach, sysctl};
use std::io;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SwapUsage {
    total: u64,
    available: u64,
    used: u64,
    page_size: u32,
    encrypted: u32,
}

pub fn collect(history: &mut History) -> io::Result<MemorySample> {
    let stats = mach::vm_statistics()?;
    let page = mach::page_size();
    let total = sysctl::value::<u64>("hw.memsize")?;
    let free = stats.free_count as u64 * page;
    let inactive = stats.inactive_count as u64 * page;
    let purgeable = stats.purgeable_count as u64 * page;
    let (available, used) = calculate_usage(total, free, inactive, purgeable);
    let wired = stats.wire_count as u64 * page;
    let compressed = stats.compressor_page_count as u64 * page;
    let cached = inactive.saturating_add(purgeable);
    let swap = sysctl::value::<SwapUsage>("vm.swapusage").unwrap_or_default();
    history.push(used as f64 * 100.0 / total.max(1) as f64);
    Ok(MemorySample {
        total,
        used,
        available,
        wired,
        compressed,
        cached,
        swap_used: swap.used,
        swap_total: swap.total,
        // macOS exposes no stable unprivileged numeric pressure API. Do not
        // mislabel a Linux-style used-memory ratio as memory pressure.
        pressure_percent: None,
        history: history.clone(),
    })
}

pub fn calculate_usage(total: u64, free: u64, inactive: u64, purgeable: u64) -> (u64, u64) {
    let available = free
        .saturating_add(inactive)
        .saturating_add(purgeable)
        .min(total);
    (available, total.saturating_sub(available))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculates_available_and_used_memory() {
        assert_eq!(calculate_usage(100, 10, 20, 5), (35, 65));
        assert_eq!(calculate_usage(100, 80, 80, 80), (100, 0));
    }
}
