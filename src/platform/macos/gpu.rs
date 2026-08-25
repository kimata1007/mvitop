use crate::metrics::gpu::GpuBackend;
use crate::model::{GpuInfo, GpuSample};
use crate::platform::macos::{iokit::Service, sysctl};
use std::io;

/// Reads the real driver-published utilization counter. Frequency, power and
/// temperature deliberately remain unavailable: macOS has no stable,
/// unprivileged API for them across Apple Silicon generations.
pub struct AppleGpuBackend {
    service: Service,
}

impl AppleGpuBackend {
    pub fn new() -> io::Result<Self> {
        Service::matching("AGXAccelerator")
            .or_else(|_| Service::matching("IOAccelerator"))
            .map(|service| Self { service })
    }
}

impl GpuBackend for AppleGpuBackend {
    fn static_info(&self) -> io::Result<GpuInfo> {
        let name = self
            .service
            .string_property("model")
            .or_else(|_| sysctl::string("machdep.cpu.brand_string"))?;
        Ok(GpuInfo { name })
    }

    fn sample(&mut self) -> io::Result<GpuSample> {
        let utilization = self
            .service
            .dictionary_number("PerformanceStatistics", "Device Utilization %")?
            .clamp(0.0, 100.0);
        Ok(GpuSample {
            utilization_percent: Some(utilization),
            ..GpuSample::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_real_gpu_registry_data_when_available() {
        let Ok(mut gpu) = AppleGpuBackend::new() else {
            return;
        };
        assert!(!gpu.static_info().unwrap().name.is_empty());
        let usage = gpu.sample().unwrap().utilization_percent.unwrap();
        assert!((0.0..=100.0).contains(&usage));
    }
}
