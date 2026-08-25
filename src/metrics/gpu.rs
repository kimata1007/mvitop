use crate::model::{GpuInfo, GpuSample};
use std::io;

pub trait GpuBackend: Send {
    fn static_info(&self) -> io::Result<GpuInfo>;
    fn sample(&mut self) -> io::Result<GpuSample>;
}

pub struct UnavailableGpu {
    reason: String,
}

impl UnavailableGpu {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl GpuBackend for UnavailableGpu {
    fn static_info(&self) -> io::Result<GpuInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            self.reason.clone(),
        ))
    }
    fn sample(&mut self) -> io::Result<GpuSample> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            self.reason.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collector_failure_is_local() {
        let mut gpu = UnavailableGpu::new("not supported");
        assert!(gpu.static_info().is_err());
        assert!(gpu.sample().is_err());
    }
}
