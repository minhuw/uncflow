//! CHA (Caching/Home Agent) register definitions for Skylake-SP
//!
//! The CHA unit manages the Last Level Cache (LLC) and handles cache coherency.
//! Each CHA unit has 4 programmable performance counters.
//!
//! ## References
//!
//! - Intel® Xeon® Processor Scalable Family Uncore Performance Monitoring Reference Manual
//! - Based on peacock C++ implementation and uncflow-agent

use crate::register::RegisterLayout;

/// Number of CHA units in Skylake-SP (up to 28, varies by SKU)
pub const CHA_COUNT: usize = 28;

/// Number of programmable counters per CHA unit
pub const COUNTERS_PER_CHA: usize = 4;

/// Bit width of CHA counters
pub const COUNTER_WIDTH_BITS: u64 = 48;

/// Stride between CHA box MSR addresses
pub const CHA_BOX_STRIDE: u64 = 0x10;

/// MSR addresses for CHA units
pub mod msr {
    use super::CHA_BOX_STRIDE;

    /// CHA Unit Box Control base address
    pub const CHA_UNIT_BOX_CTL_BASE: u64 = 0xE00;

    /// CHA Unit Counter Control 0 base address
    pub const CHA_UNIT_CTL0_BASE: u64 = 0xE01;

    /// CHA Unit Counter 0 base address
    pub const CHA_UNIT_CTR0_BASE: u64 = 0xE08;

    /// CHA Unit Filter 0 base address
    pub const CHA_UNIT_FILTER0_BASE: u64 = 0xE05;

    /// CHA Unit Filter 1 base address
    pub const CHA_UNIT_FILTER1_BASE: u64 = 0xE06;

    /// Get box control MSR address for a specific CHA
    pub const fn box_ctl(cha_index: usize) -> u64 {
        CHA_UNIT_BOX_CTL_BASE + (cha_index as u64 * CHA_BOX_STRIDE)
    }

    /// Get counter control MSR address
    pub const fn counter_ctl(cha_index: usize, counter_num: usize) -> u64 {
        CHA_UNIT_CTL0_BASE + (cha_index as u64 * CHA_BOX_STRIDE) + counter_num as u64
    }

    /// Get counter value MSR address
    pub const fn counter_value(cha_index: usize, counter_num: usize) -> u64 {
        CHA_UNIT_CTR0_BASE + (cha_index as u64 * CHA_BOX_STRIDE) + counter_num as u64
    }

    /// Get filter 0 MSR address
    pub const fn filter0(cha_index: usize) -> u64 {
        CHA_UNIT_FILTER0_BASE + (cha_index as u64 * CHA_BOX_STRIDE)
    }

    /// Get filter 1 MSR address
    pub const fn filter1(cha_index: usize) -> u64 {
        CHA_UNIT_FILTER1_BASE + (cha_index as u64 * CHA_BOX_STRIDE)
    }
}

/// CHA Unit Box Control Register layout
///
/// Controls freeze/reset for all counters in a CHA unit.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChaBoxControl {
    /// Freeze all counters in this CHA unit
    pub freeze: bool,
    /// Enable freeze control
    pub freeze_enable: bool,
    /// Reset all counters to 0
    pub reset_counters: bool,
    /// Reset all control registers
    pub reset_control: bool,
}

impl RegisterLayout for ChaBoxControl {
    fn to_msr_value(&self) -> u64 {
        let mut value = 0u64;
        if self.freeze {
            value |= 1 << 0;
        }
        if self.freeze_enable {
            value |= 1 << 8;
        }
        if self.reset_counters {
            value |= 1 << 1;
        }
        if self.reset_control {
            value |= 1 << 2;
        }
        // Unfreeze bit is at position 16
        if !self.freeze && self.freeze_enable {
            value |= 1 << 16;
        }
        value
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            freeze: (value & (1 << 0)) != 0,
            freeze_enable: (value & (1 << 8)) != 0,
            reset_counters: (value & (1 << 1)) != 0,
            reset_control: (value & (1 << 2)) != 0,
        }
    }
}

/// CHA Unit Counter Control Register layout
///
/// Controls one of the four programmable performance counters in a CHA unit.
///
/// ## Register Format
///
/// | Bits   | Field               | Description                          |
/// |--------|---------------------|--------------------------------------|
/// | 0-7    | event_select        | Event code to count                  |
/// | 8-15   | unit_mask           | Event sub-select (umask)             |
/// | 16-17  | queue_occ_select    | Queue occupancy select               |
/// | 18     | edge_detect         | Count rising edges vs level          |
/// | 19-21  | reserved            |                                      |
/// | 22     | enable              | Enable counter                       |
/// | 23     | invert              | Invert threshold comparison          |
/// | 24-29  | threshold           | Threshold for filtering (6 bits)     |
/// | 30     | occ_invert          | Invert occupancy edge                |
/// | 31     | occ_edge_detect     | Occupancy edge detect                |
#[derive(Debug, Clone, Copy, Default)]
pub struct ChaCounterControl {
    /// Event select code (bits 0-7)
    pub event_select: u8,

    /// Unit mask / event sub-select (bits 8-15)
    pub unit_mask: u8,

    /// Queue occupancy select (bits 16-17, 2 bits)
    pub queue_occupancy_select: u8,

    /// Edge detection mode (bit 18)
    pub edge_detect: bool,

    /// Enable counter (bit 22)
    pub enable: bool,

    /// Invert threshold comparison (bit 23)
    pub invert: bool,

    /// Threshold value for occupancy filtering (bits 24-29, 6 bits)
    pub threshold: u8,

    /// Invert occupancy edge (bit 30)
    pub occupancy_invert: bool,

    /// Occupancy edge detect (bit 31)
    pub occupancy_edge_detect: bool,
}

impl RegisterLayout for ChaCounterControl {
    fn to_msr_value(&self) -> u64 {
        (self.event_select as u64)
            | ((self.unit_mask as u64) << 8)
            | ((self.queue_occupancy_select as u64 & 0x03) << 16)
            | (if self.edge_detect { 1 << 18 } else { 0 })
            | (if self.enable { 1 << 22 } else { 0 })
            | (if self.invert { 1 << 23 } else { 0 })
            | ((self.threshold as u64 & 0x3F) << 24)
            | (if self.occupancy_invert { 1 << 30 } else { 0 })
            | (if self.occupancy_edge_detect {
                1 << 31
            } else {
                0
            })
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            event_select: (value & 0xFF) as u8,
            unit_mask: ((value >> 8) & 0xFF) as u8,
            queue_occupancy_select: ((value >> 16) & 0x03) as u8,
            edge_detect: (value & (1 << 18)) != 0,
            enable: (value & (1 << 22)) != 0,
            invert: (value & (1 << 23)) != 0,
            threshold: ((value >> 24) & 0x3F) as u8,
            occupancy_invert: (value & (1 << 30)) != 0,
            occupancy_edge_detect: (value & (1 << 31)) != 0,
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.threshold > 63 {
            return Err("Threshold must be <= 63 (6 bits)");
        }
        if self.queue_occupancy_select > 3 {
            return Err("Queue occupancy select must be 0-3 (2 bits)");
        }
        Ok(())
    }
}

/// CHA Unit Filter 0 Register layout
///
/// Filters events by transaction opcode matching.
///
/// ## Register Format
///
/// | Bits   | Field        | Description                    |
/// |--------|--------------|--------------------------------|
/// | 0-15   | opcode_match | Opcode to match                |
/// | 16-31  | reserved     |                                |
#[derive(Debug, Clone, Copy, Default)]
pub struct ChaFilter0 {
    /// Opcode to match (bits 0-15)
    pub opcode_match: u16,
}

impl RegisterLayout for ChaFilter0 {
    fn to_msr_value(&self) -> u64 {
        self.opcode_match as u64
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            opcode_match: (value & 0xFFFF) as u16,
        }
    }
}

/// CHA Unit Filter 1 Register layout
///
/// Filters events by cache line state.
///
/// ## Register Format
///
/// | Bits   | Field  | Description                     |
/// |--------|--------|---------------------------------|
/// | 0-16   | tid    | Thread ID filter                |
/// | 17-23  | state  | Cache line state filter         |
/// | 24-63  | reserved |                              |
#[derive(Debug, Clone, Copy, Default)]
pub struct ChaFilter1 {
    /// Thread ID filter (bits 0-16)
    pub tid: u32,

    /// Cache line state filter (bits 17-23)
    /// Bit flags for M, E, S, I states
    pub state: u8,
}

impl RegisterLayout for ChaFilter1 {
    fn to_msr_value(&self) -> u64 {
        (self.tid as u64 & 0x1FFFF) | ((self.state as u64) << 17)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            tid: (value & 0x1FFFF) as u32,
            state: ((value >> 17) & 0x7F) as u8,
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.tid > 0x1FFFF {
            return Err("TID must be <= 0x1FFFF (17 bits)");
        }
        Ok(())
    }
}

/// CHA event codes
pub mod events {
    /// TOR (Table of Requests) Occupancy
    pub const TOR_OCCUPANCY: u8 = 0x36;

    /// TOR Inserts
    pub const TOR_INSERTS: u8 = 0x35;

    /// LLC Lookup
    pub const LLC_LOOKUP: u8 = 0x34;

    /// LLC Victims (evictions)
    pub const LLC_VICTIMS: u8 = 0x37;

    /// Clockticks
    pub const CLOCKTICKS: u8 = 0x00;
}

/// CHA unit masks (event sub-selectors)
pub mod umasks {
    /// TOR occupancy/insert umasks
    pub mod tor {
        /// I/O hit
        pub const IO_HIT: u8 = 0x14;

        /// I/O miss
        pub const IO_MISS: u8 = 0x24;

        /// All requests
        pub const ALL: u8 = 0xFF;
    }

    /// LLC lookup umasks
    pub mod llc_lookup {
        /// Read lookup
        pub const READ: u8 = 0x03;

        /// Write lookup
        pub const WRITE: u8 = 0x05;

        /// Remote snoop
        pub const REMOTE_SNOOP: u8 = 0x09;

        /// Any lookup
        pub const ANY: u8 = 0x11;
    }
}

/// Cache line states for filter1 register
pub mod states {
    /// Modified
    pub const M: u8 = 0x40;

    /// Exclusive
    pub const E: u8 = 0x20;

    /// Shared
    pub const S: u8 = 0x02;

    /// Invalid
    pub const I: u8 = 0x01;

    /// Snoop Filter Modified
    pub const SFM: u8 = 0x08;

    /// Snoop Filter Exclusive
    pub const SFE: u8 = 0x04;

    /// Snoop Filter Shared
    pub const SFS: u8 = 0x02;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cha_counter_control_round_trip() {
        let ctrl = ChaCounterControl {
            event_select: 0x36,
            unit_mask: 0x14,
            enable: true,
            threshold: 10,
            ..Default::default()
        };

        let value = ctrl.to_msr_value();
        let decoded = ChaCounterControl::from_msr_value(value);

        assert_eq!(decoded.event_select, ctrl.event_select);
        assert_eq!(decoded.unit_mask, ctrl.unit_mask);
        assert_eq!(decoded.enable, ctrl.enable);
        assert_eq!(decoded.threshold, ctrl.threshold);
    }

    #[test]
    fn test_cha_validation() {
        let mut ctrl = ChaCounterControl::default();
        assert!(ctrl.validate().is_ok());

        ctrl.threshold = 64; // Too large (6 bits = max 63)
        assert!(ctrl.validate().is_err());

        ctrl.threshold = 63;
        ctrl.queue_occupancy_select = 4; // Too large (2 bits = max 3)
        assert!(ctrl.validate().is_err());
    }

    #[test]
    fn test_cha_msr_addresses() {
        assert_eq!(msr::box_ctl(0), 0xE00);
        assert_eq!(msr::box_ctl(1), 0xE10);
        assert_eq!(msr::counter_ctl(0, 0), 0xE01);
        assert_eq!(msr::counter_ctl(0, 3), 0xE04);
        assert_eq!(msr::counter_value(0, 0), 0xE08);
        assert_eq!(msr::filter0(0), 0xE05);
        assert_eq!(msr::filter1(0), 0xE06);
    }
}
