use crate::model::SystemInfo;
use crate::platform::macos::sysctl;
use std::io;
use std::time::SystemTime;

pub fn collect() -> io::Result<SystemInfo> {
    let boot = sysctl::boot_time()?;
    let (performance_cores, efficiency_cores) = core_types();
    Ok(SystemInfo {
        model: sysctl::string("hw.model").unwrap_or_else(|_| "Mac".into()),
        soc: sysctl::string("machdep.cpu.brand_string").unwrap_or_else(|_| "Apple Silicon".into()),
        os_version: sysctl::string("kern.osproductversion").unwrap_or_else(|_| "macOS".into()),
        performance_cores,
        efficiency_cores,
        uptime: SystemTime::now().duration_since(boot).unwrap_or_default(),
        load_average: sysctl::load_average().unwrap_or_default(),
    })
}

fn core_types() -> (Option<u32>, Option<u32>) {
    let levels = sysctl::value::<u32>("hw.nperflevels").unwrap_or(0);
    let mut performance = None;
    let mut efficiency = None;
    for level in 0..levels {
        let name = sysctl::string(&format!("hw.perflevel{level}.name"))
            .unwrap_or_default()
            .to_lowercase();
        let count = sysctl::value::<u32>(&format!("hw.perflevel{level}.physicalcpu")).ok();
        if name.contains("performance") {
            performance = count;
        } else if name.contains("efficiency") {
            efficiency = count;
        }
    }
    (performance, efficiency)
}
