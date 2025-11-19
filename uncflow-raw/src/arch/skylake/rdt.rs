//! RDT (Resource Director Technology) register definitions for Skylake-SP
//!
//! RDT provides cache and memory bandwidth monitoring and allocation capabilities.
//!
//! ## References
//!
//! - Intel® 64 and IA-32 Architectures Software Developer's Manual, Volume 3B
//! - Section 17.17: Intel Resource Director Technology

use crate::register::RegisterLayout;

/// MSR addresses for RDT (CMT/MBM)
pub mod msr {
    /// QM Event Select - Select monitoring event and RMID
    pub const IA32_QM_EVTSEL: u64 = 0xC8D;

    /// QM Counter - Read monitoring counter value
    pub const IA32_QM_CTR: u64 = 0xC8E;

    /// PQR Association - Associate RMID and COS with logical processor
    pub const IA32_PQR_ASSOC: u64 = 0xC8F;

    /// L3 Cache Allocation Mask - Configure L3 cache allocation
    pub const IA32_L3_QOS_MASK_BASE: u64 = 0xC90;

    /// Memory Bandwidth Allocation - Configure memory bandwidth
    pub const IA32_L2_QOS_MBA_BASE: u64 = 0xD50;
}

/// RDT monitoring event types
pub mod events {
    /// LLC Occupancy monitoring event
    pub const LLC_OCCUPANCY: u64 = 0x01;

    /// Local memory bandwidth monitoring event
    pub const LOCAL_MEM_BW: u64 = 0x02;

    /// Remote memory bandwidth monitoring event (NUMA)
    pub const REMOTE_MEM_BW: u64 = 0x03;
}

/// QM Event Select Register layout
///
/// Selects which RMID and event to monitor.
///
/// ## Register Format
///
/// | Bits   | Field     | Description                  |
/// |--------|-----------|------------------------------|
/// | 0-31   | rmid      | Resource Monitoring ID       |
/// | 32-39  | event_id  | Event ID to monitor          |
/// | 40-63  | reserved  |                              |
#[derive(Debug, Clone, Copy, Default)]
pub struct QmEventSelect {
    /// Resource Monitoring ID (RMID)
    pub rmid: u32,

    /// Event ID (LLC_OCCUPANCY, LOCAL_MEM_BW, etc.)
    pub event_id: u8,
}

impl RegisterLayout for QmEventSelect {
    fn to_msr_value(&self) -> u64 {
        (self.rmid as u64) | ((self.event_id as u64) << 32)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            rmid: (value & 0xFFFFFFFF) as u32,
            event_id: ((value >> 32) & 0xFF) as u8,
        }
    }
}

/// PQR Association Register layout
///
/// Associates an RMID and COS (Class of Service) with the current logical processor.
///
/// ## Register Format
///
/// | Bits   | Field     | Description                  |
/// |--------|-----------|------------------------------|
/// | 0-31   | rmid      | Resource Monitoring ID       |
/// | 32-63  | cos       | Class of Service             |
#[derive(Debug, Clone, Copy, Default)]
pub struct PqrAssoc {
    /// Resource Monitoring ID (RMID) for this logical processor
    pub rmid: u32,

    /// Class of Service (COS) for cache/memory allocation
    pub cos: u32,
}

impl RegisterLayout for PqrAssoc {
    fn to_msr_value(&self) -> u64 {
        (self.rmid as u64) | ((self.cos as u64) << 32)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            rmid: (value & 0xFFFFFFFF) as u32,
            cos: ((value >> 32) & 0xFFFFFFFF) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qm_event_select_round_trip() {
        let evtsel = QmEventSelect {
            rmid: 42,
            event_id: events::LLC_OCCUPANCY as u8,
        };

        let value = evtsel.to_msr_value();
        let decoded = QmEventSelect::from_msr_value(value);

        assert_eq!(decoded.rmid, evtsel.rmid);
        assert_eq!(decoded.event_id, evtsel.event_id);
    }

    #[test]
    fn test_pqr_assoc_round_trip() {
        let pqr = PqrAssoc { rmid: 10, cos: 5 };

        let value = pqr.to_msr_value();
        let decoded = PqrAssoc::from_msr_value(value);

        assert_eq!(decoded.rmid, pqr.rmid);
        assert_eq!(decoded.cos, pqr.cos);
    }
}
