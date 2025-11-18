use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;

use crate::common::affinity::AffinityGuard;
use crate::error::{Result, UncflowError};

pub struct MsrHandle {
    file: parking_lot::Mutex<File>,
    cpu_id: u32,
}

impl MsrHandle {
    pub fn new(cpu: u32) -> Result<Self> {
        let path = format!("/dev/cpu/{cpu}/msr");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                UncflowError::MsrError(format!("Failed to open {path} for CPU {cpu}: {e}"))
            })?;

        tracing::info!("Opened MSR handle {} for core {}", file.as_raw_fd(), cpu);

        Ok(Self {
            file: parking_lot::Mutex::new(file),
            cpu_id: cpu,
        })
    }

    pub fn read(&self, addr: u64) -> Result<u64> {
        let _affinity = AffinityGuard::new(self.cpu_id as i32)?;
        let mut file = self.file.lock();

        file.seek(SeekFrom::Start(addr)).map_err(|e| {
            UncflowError::MsrError(format!(
                "Failed to seek to MSR 0x{:X} on CPU {}: {}",
                addr, self.cpu_id, e
            ))
        })?;

        let mut buffer = [0u8; 8];
        file.read_exact(&mut buffer).map_err(|e| {
            UncflowError::MsrError(format!(
                "Failed to read MSR 0x{:X} on CPU {}: {}",
                addr, self.cpu_id, e
            ))
        })?;

        let value = u64::from_ne_bytes(buffer);
        tracing::debug!(
            "MSR read: CPU {} MSR 0x{:08x} = 0x{:016x}",
            self.cpu_id,
            addr,
            value
        );
        Ok(value)
    }

    pub fn write(&self, addr: u64, value: u64) -> Result<()> {
        let _affinity = AffinityGuard::new(self.cpu_id as i32)?;
        let mut file = self.file.lock();

        file.seek(SeekFrom::Start(addr)).map_err(|e| {
            UncflowError::MsrError(format!(
                "Failed to seek to MSR 0x{:X} on CPU {}: {}",
                addr, self.cpu_id, e
            ))
        })?;

        file.write_all(&value.to_ne_bytes()).map_err(|e| {
            UncflowError::MsrError(format!(
                "Failed to write MSR 0x{:X} on CPU {}: {}",
                addr, self.cpu_id, e
            ))
        })?;

        Ok(())
    }

    pub fn cpu_id(&self) -> u32 {
        self.cpu_id
    }
}

pub struct Msr {
    handles: RwLock<HashMap<u32, Arc<MsrHandle>>>,
}

impl Msr {
    fn new() -> Self {
        Self {
            handles: RwLock::new(HashMap::new()),
        }
    }

    pub fn instance() -> &'static Msr {
        static INSTANCE: Lazy<Msr> = Lazy::new(Msr::new);
        &INSTANCE
    }

    fn get_handle(&self, cpu: u32) -> Result<Arc<MsrHandle>> {
        {
            let handles = self.handles.read();
            if let Some(handle) = handles.get(&cpu) {
                return Ok(Arc::clone(handle));
            }
        }

        let mut handles = self.handles.write();
        if let Some(handle) = handles.get(&cpu) {
            return Ok(Arc::clone(handle));
        }

        let handle = Arc::new(MsrHandle::new(cpu)?);
        handles.insert(cpu, Arc::clone(&handle));
        Ok(handle)
    }

    pub fn read(&self, cpu: u32, addr: u64) -> Result<u64> {
        let handle = self.get_handle(cpu)?;
        handle.read(addr)
    }

    pub fn write(&self, cpu: u32, addr: u64, value: u64) -> Result<()> {
        let handle = self.get_handle(cpu)?;
        handle.write(addr, value)
    }
}

pub fn read(cpu: u32, addr: u64) -> Result<u64> {
    Msr::instance().read(cpu, addr)
}

pub fn write(cpu: u32, addr: u64, value: u64) -> Result<()> {
    Msr::instance().write(cpu, addr, value)
}

pub fn read_msr(cpu: u32, addr: u64) -> Result<u64> {
    Msr::instance().read(cpu, addr)
}

pub fn write_msr(cpu: u32, addr: u64, value: u64) -> Result<()> {
    Msr::instance().write(cpu, addr, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msr_singleton() {
        let msr1 = Msr::instance();
        let msr2 = Msr::instance();
        assert!(std::ptr::eq(msr1, msr2));
    }
}
