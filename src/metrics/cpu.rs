use crate::model::CpuSample;
use crate::platform::macos::mach::{self, CpuTicks};
use std::io;

#[derive(Default)]
pub struct CpuCollector {
    previous: Vec<CpuTicks>,
}

impl CpuCollector {
    pub fn sample(&mut self, history: &mut crate::model::History) -> io::Result<CpuSample> {
        let current = mach::cpu_ticks()?;
        let per_core_percent = utilization(&self.previous, &current);
        self.previous = current;
        let total_percent = if per_core_percent.is_empty() {
            0.0
        } else {
            per_core_percent.iter().sum::<f64>() / per_core_percent.len() as f64
        };
        history.push(total_percent);
        Ok(CpuSample {
            total_percent,
            per_core_percent,
            history: history.clone(),
        })
    }
}

pub fn utilization(previous: &[CpuTicks], current: &[CpuTicks]) -> Vec<f64> {
    if previous.len() != current.len() {
        return vec![0.0; current.len()];
    }
    previous
        .iter()
        .zip(current)
        .map(|(old, new)| {
            let busy = new.user.saturating_sub(old.user)
                + new.system.saturating_sub(old.system)
                + new.nice.saturating_sub(old.nice);
            let idle = new.idle.saturating_sub(old.idle);
            let total = busy + idle;
            if total == 0 {
                0.0
            } else {
                busy as f64 * 100.0 / total as f64
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculates_cpu_deltas() {
        let old = [CpuTicks {
            user: 10,
            system: 10,
            idle: 80,
            nice: 0,
        }];
        let new = [CpuTicks {
            user: 30,
            system: 20,
            idle: 150,
            nice: 0,
        }];
        assert!((utilization(&old, &new)[0] - 30.0).abs() < 0.001);
    }
    #[test]
    fn handles_counter_regression() {
        let old = [CpuTicks {
            user: 100,
            system: 100,
            idle: 100,
            nice: 0,
        }];
        let new = [CpuTicks::default()];
        assert_eq!(utilization(&old, &new), vec![0.0]);
    }
}
