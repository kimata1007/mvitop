use crate::model::SystemInfo;
use crate::platform::macos::sysctl;
use std::io;
use std::time::SystemTime;

pub fn collect() -> io::Result<SystemInfo> {
    let boot = sysctl::boot_time()?;
    Ok(SystemInfo {
        model: sysctl::string("hw.model").unwrap_or_else(|_| "Mac".into()),
        soc: sysctl::string("machdep.cpu.brand_string").unwrap_or_else(|_| "Apple Silicon".into()),
        os_version: sysctl::string("kern.osproductversion").unwrap_or_else(|_| "macOS".into()),
        uptime: SystemTime::now().duration_since(boot).unwrap_or_default(),
        load_average: sysctl::load_average().unwrap_or_default(),
    })
}

pub fn refresh_slow(info: &mut SystemInfo) {
    if let Ok(boot) = sysctl::boot_time() {
        info.uptime = SystemTime::now().duration_since(boot).unwrap_or_default();
    }
    if let Ok(load) = sysctl::load_average() {
        info.load_average = load;
    }
}
