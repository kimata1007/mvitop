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
    let available = free
        .saturating_add(inactive)
        .saturating_add(purgeable)
        .min(total);
    let used = total.saturating_sub(available);
    let wired = stats.wire_count as u64 * page;
    let compressed = stats.compressor_page_count as u64 * page;
    let cached = inactive.saturating_add(purgeable);
    let pressure_percent = pressure(total, stats.active_count as u64 * page, wired, compressed);
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
        pressure_percent,
        history: history.clone(),
    })
}

pub fn pressure(total: u64, active: u64, wired: u64, compressed: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((active.saturating_add(wired).saturating_add(compressed)).min(total) as f64 * 100.0
        / total as f64)
        .clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_pressure_is_bounded() {
        assert_eq!(pressure(100, 50, 30, 10), 90.0);
        assert_eq!(pressure(100, 100, 100, 100), 100.0);
        assert_eq!(pressure(0, 1, 1, 1), 0.0);
    }
}
