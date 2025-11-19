//! IIO (Integrated I/O) register definitions for Skylake-SP
//!
//! The IIO unit handles PCIe root complex functionality and I/O traffic monitoring.
//!
//! ## References
//!
//! - Based on peacock C++ implementation
//! - Intel® Xeon® Processor Scalable Family Uncore Performance Monitoring Reference Manual

use crate::register::RegisterLayout;

/// Number of IIO channels per socket (Skylake-SP has 3 IIO stacks)
pub const IIO_CHANNEL_COUNT: usize = 3;

/// Number of PCIe ports per IIO channel
pub const IIO_PCIE_PORT_COUNT: usize = 4;

/// Number of programmable counters per IIO unit
pub const IIO_COUNTERS_PER_UNIT: usize = 4;

/// Bit width of IIO free-running counters (PCIe bandwidth counters)
pub const IIO_COUNTER_WIDTH_BITS: u64 = 36;

/// Bit width of IIO programmable counters
pub const UNCORE_COUNTER_WIDTH_BITS: u64 = 48;

/// MSR addresses for IIO units
pub mod msr {
    /// IIO Unit Box Control registers (one per IIO channel)
    pub const IIO_UNIT_BOX_CTL: [u64; 3] = [0x0A60, 0x0A80, 0x0AA0];

    /// IIO Unit Box Status registers (one per IIO channel)
    pub const IIO_UNIT_BOX_STATUS: [u64; 3] = [0x0A67, 0x0A87, 0x0AA7];

    /// IIO Unit Counter Control Register 0 (one per IIO channel)
    pub const IIO_UNIT_CTL0: [u64; 3] = [0x0A68, 0x0A88, 0x0AA8];

    /// IIO Unit Counter Control Register 1
    pub const IIO_UNIT_CTL1: [u64; 3] = [0x0A69, 0x0A89, 0x0AA9];

    /// IIO Unit Counter Control Register 2
    pub const IIO_UNIT_CTL2: [u64; 3] = [0x0A6A, 0x0A8A, 0x0AAA];

    /// IIO Unit Counter Control Register 3
    pub const IIO_UNIT_CTL3: [u64; 3] = [0x0A6B, 0x0A8B, 0x0AAB];

    /// IIO Unit Counter 0 Value
    pub const IIO_UNIT_CTR0: [u64; 3] = [0x0A61, 0x0A81, 0x0AA1];

    /// IIO Unit Counter 1 Value
    pub const IIO_UNIT_CTR1: [u64; 3] = [0x0A62, 0x0A82, 0x0AA2];

    /// IIO Unit Counter 2 Value
    pub const IIO_UNIT_CTR2: [u64; 3] = [0x0A63, 0x0A83, 0x0AA3];

    /// IIO Unit Counter 3 Value
    pub const IIO_UNIT_CTR3: [u64; 3] = [0x0A64, 0x0A84, 0x0AA4];

    /// IIO Unit Clock Counter (uncore clock)
    pub const IIO_UNIT_CLK: [u64; 3] = [0x0A65, 0x0A85, 0x0AA5];

    /// PCIe free-running bandwidth counters - Inbound
    /// [channel][port]
    pub const IIO_PCIE_BANDWIDTH_IN: [[u64; 4]; 3] = [
        [0x0B10, 0x0B11, 0x0B12, 0x0B13],
        [0x0B20, 0x0B21, 0x0B22, 0x0B23],
        [0x0B30, 0x0B31, 0x0B32, 0x0B33],
    ];

    /// PCIe free-running bandwidth counters - Outbound
    /// [channel][port]
    pub const IIO_PCIE_BANDWIDTH_OUT: [[u64; 4]; 3] = [
        [0x0B14, 0x0B15, 0x0B16, 0x0B17],
        [0x0B24, 0x0B25, 0x0B26, 0x0B27],
        [0x0B34, 0x0B35, 0x0B36, 0x0B37],
    ];
}

/// IIO Unit Counter Control Register layout
///
/// This register controls one of the four programmable performance counters
/// in an IIO unit. The bit layout is verified from the peacock C++ implementation.
///
/// ## Register Format (47 bits used)
///
/// | Bits   | Field               | Description                          |
/// |--------|---------------------|--------------------------------------|
/// | 0-7    | event_select        | Event code to count                  |
/// | 8-15   | unit_mask           | Event sub-select (umask)             |
/// | 16     | reserved            | Must be 0                            |
/// | 17     | reset_counter       | Reset counter on programming         |
/// | 18     | edge_detect         | Count rising edges vs level          |
/// | 19     | thread_id_enable    | Enable thread ID filtering           |
/// | 20     | overflow_enable     | Enable overflow interrupts           |
/// | 21     | reserved            | Must be 0                            |
/// | 22     | enable              | Enable counter                       |
/// | 23     | invert              | Invert threshold comparison          |
/// | 24-35  | threshold           | Threshold for filtering (12 bits)    |
/// | 36-43  | channel_mask        | Channel filter mask (8 bits)         |
/// | 44-46  | fc_mask             | Fabric config filter mask (3 bits)   |
/// | 47-63  | reserved            | Must be 0                            |
///
/// ## Example
///
/// ```ignore
/// use uncflow_raw::arch::skylake::iio::IioCounterControl;
/// use uncflow_raw::RegisterLayout;
///
/// let ctrl = IioCounterControl {
///     event_select: 0x41,  // IIO TLB event
///     unit_mask: 0x20,     // TLB miss
///     reset_counter: true,
///     overflow_enable: true,
///     enable: true,
///     channel_mask: 0xFF,  // All channels
///     fc_mask: 0x07,       // All fabric configs
///     ..Default::default()
/// };
///
/// let msr_value = ctrl.to_msr_value();
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct IioCounterControl {
    /// Event select code (bits 0-7)
    pub event_select: u8,

    /// Unit mask / event sub-select (bits 8-15)
    pub unit_mask: u8,

    /// Reset counter on programming (bit 16)
    pub reset_counter: bool,

    /// Edge detection mode (bit 17)
    pub edge_detect: bool,

    /// Thread ID filtering enable (bit 18)
    pub thread_id_enable: bool,

    /// Overflow interrupt enable (bit 19)
    pub overflow_enable: bool,

    /// Enable counter (bit 21)
    pub enable: bool,

    /// Invert threshold comparison (bit 22)
    pub invert: bool,

    /// Threshold value for occupancy filtering (bits 23-34, 12 bits)
    pub threshold: u16,

    /// Channel mask - which IIO channels to monitor (bits 35-42, 8 bits)
    /// Set bit N to monitor channel N. 0xFF = all channels.
    pub channel_mask: u8,

    /// Fabric configuration mask (bits 43-45, 3 bits)
    /// Filters by fabric request type. 0x07 = all types.
    pub fc_mask: u8,
}

impl RegisterLayout for IioCounterControl {
    fn to_msr_value(&self) -> u64 {
        (self.event_select as u64)
            | ((self.unit_mask as u64) << 8)
            // Bit 16 is reserved
            | (if self.reset_counter { 1 << 17 } else { 0 })
            | (if self.edge_detect { 1 << 18 } else { 0 })
            | (if self.thread_id_enable { 1 << 19 } else { 0 })
            | (if self.overflow_enable { 1 << 20 } else { 0 })
            // Bit 21 is reserved
            | (if self.enable { 1 << 22 } else { 0 })
            | (if self.invert { 1 << 23 } else { 0 })
            | ((self.threshold as u64 & 0xFFF) << 24)
            | ((self.channel_mask as u64) << 36)
            | ((self.fc_mask as u64 & 0x07) << 44)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            event_select: (value & 0xFF) as u8,
            unit_mask: ((value >> 8) & 0xFF) as u8,
            reset_counter: (value & (1 << 17)) != 0,
            edge_detect: (value & (1 << 18)) != 0,
            thread_id_enable: (value & (1 << 19)) != 0,
            overflow_enable: (value & (1 << 20)) != 0,
            enable: (value & (1 << 22)) != 0,
            invert: (value & (1 << 23)) != 0,
            threshold: ((value >> 24) & 0xFFF) as u16,
            channel_mask: ((value >> 36) & 0xFF) as u8,
            fc_mask: ((value >> 44) & 0x07) as u8,
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.threshold > 0xFFF {
            return Err("Threshold must be <= 4095 (12 bits)");
        }
        if self.fc_mask > 0x07 {
            return Err("FC mask must be <= 7 (3 bits)");
        }
        Ok(())
    }
}

/// IIO event codes
pub mod events {
    /// IIO TLB-related events
    pub const IIO_TLB_EVENT: u8 = 0x41;

    /// IIO queue occupancy
    pub const IIO_OCCUPANCY: u8 = 0x40;

    /// IIO completion inserts
    pub const IIO_COMP_INSERTS: u8 = 0xC2;

    /// IIO completion occupancy
    pub const IIO_COMP_OCCUPANCY: u8 = 0xD5;

    /// Uncore clockticks
    pub const CLOCKTICKS: u8 = 0x01;
}

/// IIO unit masks (event sub-selectors)
pub mod umasks {
    /// TLB hit
    pub const TLB_HIT: u8 = 0x01;

    /// TLB context miss
    pub const TLB_CONTEXT_MISS: u8 = 0x02;

    /// TLB L1 miss
    pub const TLB_L1_MISS: u8 = 0x04;

    /// TLB L2 miss
    pub const TLB_L2_MISS: u8 = 0x08;

    /// TLB L3 miss
    pub const TLB_L3_MISS: u8 = 0x10;

    /// TLB miss (all levels)
    pub const TLB_MISS_ALL: u8 = 0x20;

    /// TLB full condition
    pub const TLB_FULL: u8 = 0x40;

    /// TLB1 miss
    pub const TLB1_MISS: u8 = 0x80;

    /// Completion inserts umask
    pub const COMP_INSERTS: u8 = 0x04;

    /// All channels mask (use with channel_mask field)
    pub const CH_MASK_ALL: u8 = 0xFF;

    /// All fabric configs (use with fc_mask field)
    pub const FC_MASK_ALL: u8 = 0x07;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iio_counter_control_round_trip() {
        let ctrl = IioCounterControl {
            event_select: 0x41,
            unit_mask: 0x20,
            reset_counter: true,
            overflow_enable: true,
            enable: true,
            channel_mask: 0xFF,
            fc_mask: 0x07,
            threshold: 100,
            ..Default::default()
        };

        let value = ctrl.to_msr_value();
        let decoded = IioCounterControl::from_msr_value(value);

        assert_eq!(decoded.event_select, ctrl.event_select);
        assert_eq!(decoded.unit_mask, ctrl.unit_mask);
        assert_eq!(decoded.reset_counter, ctrl.reset_counter);
        assert_eq!(decoded.overflow_enable, ctrl.overflow_enable);
        assert_eq!(decoded.enable, ctrl.enable);
        assert_eq!(decoded.channel_mask, ctrl.channel_mask);
        assert_eq!(decoded.fc_mask, ctrl.fc_mask);
        assert_eq!(decoded.threshold, ctrl.threshold);
    }

    #[test]
    fn test_iio_validation() {
        let mut ctrl = IioCounterControl::default();
        assert!(ctrl.validate().is_ok());

        ctrl.threshold = 0x1000; // Too large (12 bits = max 0xFFF)
        assert!(ctrl.validate().is_err());

        ctrl.threshold = 0xFFF;
        ctrl.fc_mask = 0x08; // Too large (3 bits = max 0x07)
        assert!(ctrl.validate().is_err());
    }
}
