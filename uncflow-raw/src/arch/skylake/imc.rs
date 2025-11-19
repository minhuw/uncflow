//! IMC (Integrated Memory Controller) register definitions for Skylake-SP
//!
//! The IMC unit manages memory controllers and provides performance counters for
//! monitoring memory bandwidth, latency, and queue occupancy.
//!
//! ## References
//!
//! - Intel® Xeon® Processor Scalable Family Uncore Performance Monitoring Reference Manual
//! - Section: Memory Controller Performance Monitoring

/// Number of IMC channels in Skylake-SP (varies by SKU, up to 6)
pub const IMC_CHANNEL_COUNT: usize = 6;

/// Number of programmable counters per IMC channel
pub const COUNTERS_PER_CHANNEL: usize = 4;

/// Bit width of IMC counters (48 bits)
pub const COUNTER_WIDTH_BITS: u64 = 48;

/// Cache line size for bandwidth calculations (64 bytes)
pub const CACHE_LINE_SIZE: u64 = 64;

/// MSR addresses for IMC performance counters
pub mod msr {
    /// IMC Unit Control Register
    pub const IMC_UNIT_CTRL: u64 = 0x0F1;

    /// IMC Counter 0 (per channel)
    pub const IMC_CTR0: u64 = 0x0A0;

    /// IMC Counter 1 (per channel)
    pub const IMC_CTR1: u64 = 0x0A8;

    /// IMC Counter 2 (per channel)
    pub const IMC_CTR2: u64 = 0x0B0;

    /// IMC Counter 3 (per channel)
    pub const IMC_CTR3: u64 = 0x0B8;

    /// IMC Control 0 (per channel)
    pub const IMC_CTL0: u64 = 0x0D8;

    /// IMC Control 1 (per channel)
    pub const IMC_CTL1: u64 = 0x0DC;

    /// IMC Control 2 (per channel)
    pub const IMC_CTL2: u64 = 0x0E0;

    /// IMC Control 3 (per channel)
    pub const IMC_CTL3: u64 = 0x0E4;
}

/// PCI configuration addresses for IMC channels
///
/// Skylake-SP uses PCI configuration space for IMC access.
/// Format: (device, function, device_id)
pub mod pci {
    /// IMC Box Control register offset
    pub const IMC_BOX_CTL: u32 = 0x0F4;

    /// IMC DCLK Control register offset
    pub const IMC_DCLK_CTL: u32 = 0x0A4;

    /// IMC DCLK Counter offset
    pub const IMC_DCLK_CTR: u32 = 0x0A4;

    /// IMC channel PCI configurations: (device, function, device_id)
    ///
    /// Each entry represents: (PCI device, PCI function, Device ID)
    pub const IMC_CHANNELS: [(u32, u32, u32); 6] = [
        (0x0A, 2, 0x2042), // Channel 0: device 10, function 2
        (0x0A, 6, 0x2046), // Channel 1: device 10, function 6
        (0x0B, 2, 0x204A), // Channel 2: device 11, function 2
        (0x0C, 2, 0x2042), // Channel 3: device 12, function 2
        (0x0C, 6, 0x2046), // Channel 4: device 12, function 6
        (0x0D, 2, 0x204A), // Channel 5: device 13, function 2
    ];
}

/// IMC performance event codes
pub mod events {
    /// CAS Count Read event select
    pub const CAS_COUNT_RD: u8 = 0x04;

    /// CAS Count Write event select
    pub const CAS_COUNT_WR: u8 = 0x04;

    /// Umask for read CAS operations
    pub const CAS_COUNT_RD_UMASK: u8 = 0x03;

    /// Umask for write CAS operations
    pub const CAS_COUNT_WR_UMASK: u8 = 0x0C;

    /// Read Pending Queue occupancy event
    pub const RPQ_OCCUPANCY: u8 = 0x80;

    /// Write Pending Queue occupancy event
    pub const WPQ_OCCUPANCY: u8 = 0x81;
}
