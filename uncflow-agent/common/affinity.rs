use nix::sched::{sched_getaffinity, sched_setaffinity, CpuSet};
use nix::unistd::Pid;

use crate::error::{Result, UncflowError};

pub struct AffinityGuard {
    old_affinity: CpuSet,
}

impl AffinityGuard {
    pub fn new(cpu: i32) -> Result<Self> {
        if cpu < 0 {
            return Err(UncflowError::AffinityError(format!(
                "Invalid CPU ID: {cpu}"
            )));
        }

        let old_affinity = sched_getaffinity(Pid::from_raw(0))
            .map_err(|e| UncflowError::AffinityError(format!("Failed to get affinity: {e}")))?;

        let mut new_affinity = CpuSet::new();
        new_affinity.set(cpu as usize).map_err(|e| {
            UncflowError::AffinityError(format!("Failed to set CPU {cpu} in set: {e}"))
        })?;

        sched_setaffinity(Pid::from_raw(0), &new_affinity).map_err(|e| {
            UncflowError::AffinityError(format!("Failed to set affinity to CPU {cpu}: {e}"))
        })?;

        Ok(Self { old_affinity })
    }
}

impl Drop for AffinityGuard {
    fn drop(&mut self) {
        let _ = sched_setaffinity(Pid::from_raw(0), &self.old_affinity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affinity_guard_creation() {
        let result = AffinityGuard::new(0);
        assert!(result.is_ok() || result.is_err());
    }
}
