//! IRP (I/O Request Processing) register definitions for Skylake-SP
//!
//! The IRP unit handles I/O requests and provides performance counters for
//! monitoring PCIe bandwidth, latency, and various I/O operations.
//!
//! ## References
//!
//! - Intel® Xeon® Processor Scalable Family Uncore Performance Monitoring Reference Manual
//! - Section: I/O Request Processing Performance Monitoring

/// Number of IRP units in Skylake-SP
pub const IRP_UNIT_COUNT: usize = 3;

/// Number of programmable counters per IRP unit
pub const COUNTERS_PER_IRP: usize = 4;

/// Bit width of IRP counters
pub const COUNTER_WIDTH_BITS: u64 = 48;

/// IRP PCI Device ID for Skylake-SP
pub const IRP_DEVICE_ID: u32 = 0x6F39;

/// MSR addresses for IRP performance counters
pub mod msr {
    /// IRP Unit Control registers (one per IRP unit)
    pub const IRP_UNIT_CTRL: [u64; 3] = [0x0A78, 0x0A98, 0x0AB8];

    /// IRP Unit Status registers (one per IRP unit)
    pub const IRP_UNIT_STATUS: [u64; 3] = [0x0A7F, 0x0A9F, 0x0ABF];

    /// IRP Counter 0 registers (one per IRP unit)
    pub const IRP_CTR0: [u64; 3] = [0x0A79, 0x0A99, 0x0AB9];

    /// IRP Counter 1 registers (one per IRP unit)
    pub const IRP_CTR1: [u64; 3] = [0x0A7A, 0x0A9A, 0x0ABA];

    /// IRP Control 0 registers (one per IRP unit)
    pub const IRP_CTRL0: [u64; 3] = [0x0A7B, 0x0A9B, 0x0ABB];

    /// IRP Control 1 registers (one per IRP unit)
    pub const IRP_CTRL1: [u64; 3] = [0x0A7C, 0x0A9C, 0x0ABC];
}

/// PCI configuration addresses for IRP units
pub mod pci {
    /// IRP Unit Status register offset
    pub const IRP_UNIT_STATUS_ADDR: u32 = 0xF8;

    /// IRP Unit Control register offset
    pub const IRP_UNIT_CTL_ADDR: u32 = 0xF4;

    /// IRP Counter register offsets (4 counters)
    pub const IRP_CTR_ADDR: [u32; 4] = [0xA0, 0xB0, 0xB8, 0xC0];

    /// IRP Control register offsets (4 control registers)
    pub const IRP_CTL_ADDR: [u32; 4] = [0xD8, 0xDC, 0xE0, 0xE4];
}
