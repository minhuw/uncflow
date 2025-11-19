//! MSR (Model-Specific Register) read/write primitives
//!
//! This module provides low-level MSR access through `/dev/cpu/*/msr`.
//! For cached/pooled access, use the higher-level abstractions in uncflow-agent.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;

pub type Result<T> = std::result::Result<T, MsrError>;

/// Errors that can occur during MSR operations
#[derive(Debug, thiserror::Error)]
pub enum MsrError {
    #[error("Failed to open MSR device for CPU {cpu}: {source}")]
    OpenFailed { cpu: u32, source: std::io::Error },

    #[error("Failed to read MSR 0x{msr:X} on CPU {cpu}: {source}")]
    ReadFailed {
        cpu: u32,
        msr: u64,
        source: std::io::Error,
    },

    #[error("Failed to write MSR 0x{msr:X} on CPU {cpu}: {source}")]
    WriteFailed {
        cpu: u32,
        msr: u64,
        source: std::io::Error,
    },

    #[error("Failed to seek to MSR 0x{msr:X} on CPU {cpu}: {source}")]
    SeekFailed {
        cpu: u32,
        msr: u64,
        source: std::io::Error,
    },
}

/// Read a 64-bit value from an MSR
///
/// # Arguments
///
/// * `cpu` - CPU core number (0-indexed)
/// * `msr` - MSR address (e.g., 0xE01 for CHA_UNIT_CTL0)
///
/// # Errors
///
/// Returns an error if:
/// - The MSR device cannot be opened (requires root/CAP_SYS_RAWIO)
/// - The MSR address is invalid
/// - The MSR is not readable
///
/// # Example
///
/// ```ignore
/// use uncflow_raw::read_msr;
///
/// let value = read_msr(0, 0xE01)?;
/// println!("MSR 0xE01 = 0x{:016X}", value);
/// ```
pub fn read_msr(cpu: u32, msr: u64) -> Result<u64> {
    let path = format!("/dev/cpu/{cpu}/msr");
    let mut file = File::open(&path).map_err(|e| MsrError::OpenFailed { cpu, source: e })?;

    file.seek(SeekFrom::Start(msr))
        .map_err(|e| MsrError::SeekFailed {
            cpu,
            msr,
            source: e,
        })?;

    let mut buffer = [0u8; 8];
    file.read_exact(&mut buffer)
        .map_err(|e| MsrError::ReadFailed {
            cpu,
            msr,
            source: e,
        })?;

    Ok(u64::from_le_bytes(buffer))
}

/// Write a 64-bit value to an MSR
///
/// # Arguments
///
/// * `cpu` - CPU core number (0-indexed)
/// * `msr` - MSR address (e.g., 0xE01 for CHA_UNIT_CTL0)
/// * `value` - 64-bit value to write
///
/// # Errors
///
/// Returns an error if:
/// - The MSR device cannot be opened (requires root/CAP_SYS_RAWIO)
/// - The MSR address is invalid
/// - The MSR is read-only
/// - The value contains invalid bits (reserved/undefined)
///
/// # Safety
///
/// Writing incorrect values to MSRs can cause system instability or crashes.
/// Always validate register values using `RegisterLayout::validate()` before writing.
///
/// # Example
///
/// ```ignore
/// use uncflow_raw::write_msr;
/// use uncflow_raw::current_arch::cha::ChaCounterControl;
/// use uncflow_raw::RegisterLayout;
///
/// let ctrl = ChaCounterControl {
///     event_select: 0x34,
///     enable: true,
///     ..Default::default()
/// };
///
/// // Validate before writing
/// ctrl.validate()?;
///
/// write_msr(0, 0xE01, ctrl.to_msr_value())?;
/// ```
pub fn write_msr(cpu: u32, msr: u64, value: u64) -> Result<()> {
    let path = format!("/dev/cpu/{cpu}/msr");
    let mut file = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_SYNC) // Ensure synchronous writes
        .open(&path)
        .map_err(|e| MsrError::OpenFailed { cpu, source: e })?;

    file.seek(SeekFrom::Start(msr))
        .map_err(|e| MsrError::SeekFailed {
            cpu,
            msr,
            source: e,
        })?;

    file.write_all(&value.to_le_bytes())
        .map_err(|e| MsrError::WriteFailed {
            cpu,
            msr,
            source: e,
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msr_error_display() {
        let err = MsrError::OpenFailed {
            cpu: 0,
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        assert!(err.to_string().contains("Failed to open MSR device"));
    }
}
