//! Core PMU register definitions for Skylake-SP
//!
//! Core Performance Monitoring Unit for monitoring core-level events.
//!
//! ## References
//!
//! - Intel® 64 and IA-32 Architectures Software Developer's Manual, Volume 3B
//! - Chapter 18: Performance Monitoring

use crate::register::RegisterLayout;

/// Number of general-purpose performance counters per core
pub const CORE_PMU_COUNTERS: usize = 4;

/// Number of fixed-function performance counters
pub const CORE_FIXED_COUNTERS: usize = 3;

/// MSR addresses for Core PMU
pub mod msr {
    /// Performance Event Select registers (IA32_PERFEVTSELx)
    pub const IA32_PERFEVTSEL0: u64 = 0x186;
    pub const IA32_PERFEVTSEL1: u64 = 0x187;
    pub const IA32_PERFEVTSEL2: u64 = 0x188;
    pub const IA32_PERFEVTSEL3: u64 = 0x189;

    /// Performance Counter registers (IA32_PMCx)
    pub const IA32_PMC0: u64 = 0xC1;
    pub const IA32_PMC1: u64 = 0xC2;
    pub const IA32_PMC2: u64 = 0xC3;
    pub const IA32_PMC3: u64 = 0xC4;

    /// Fixed-function Performance Counter Control
    pub const IA32_FIXED_CTR_CTRL: u64 = 0x38D;

    /// Fixed-function Performance Counters
    pub const IA32_FIXED_CTR0: u64 = 0x309; // Instructions Retired
    pub const IA32_FIXED_CTR1: u64 = 0x30A; // Core Cycles
    pub const IA32_FIXED_CTR2: u64 = 0x30B; // Reference Cycles

    /// Performance Counter Global Control
    pub const IA32_PERF_GLOBAL_CTRL: u64 = 0x38F;

    /// Performance Counter Global Status
    pub const IA32_PERF_GLOBAL_STATUS: u64 = 0x38E;

    /// Performance Counter Global Status Reset
    pub const IA32_PERF_GLOBAL_STATUS_RESET: u64 = 0x390;
}

/// Core Performance Event Select Register layout
///
/// ## Register Format
///
/// | Bits   | Field       | Description                    |
/// |--------|-------------|--------------------------------|
/// | 0-7    | event_select| Event select                   |
/// | 8-15   | umask       | Unit mask                      |
/// | 16     | usr         | User mode                      |
/// | 17     | os          | OS mode                        |
/// | 18     | edge        | Edge detect                    |
/// | 19     | pc          | Pin control                    |
/// | 20     | int         | APIC interrupt enable          |
/// | 21     | any_thread  | Any thread                     |
/// | 22     | enable      | Enable counter                 |
/// | 23     | invert      | Invert counter mask            |
/// | 24-31  | cmask       | Counter mask                   |
#[derive(Debug, Clone, Copy, Default)]
pub struct CorePerfEvtSel {
    /// Event select (bits 0-7)
    pub event_select: u8,

    /// Unit mask (bits 8-15)
    pub umask: u8,

    /// Count in user mode (bit 16)
    pub usr: bool,

    /// Count in OS mode (bit 17)
    pub os: bool,

    /// Edge detect (bit 18)
    pub edge: bool,

    /// Pin control (bit 19)
    pub pc: bool,

    /// APIC interrupt enable (bit 20)
    pub int: bool,

    /// Any thread (bit 21)
    pub any_thread: bool,

    /// Enable counter (bit 22)
    pub enable: bool,

    /// Invert counter mask (bit 23)
    pub invert: bool,

    /// Counter mask (bits 24-31)
    pub cmask: u8,
}

impl RegisterLayout for CorePerfEvtSel {
    fn to_msr_value(&self) -> u64 {
        (self.event_select as u64)
            | ((self.umask as u64) << 8)
            | (if self.usr { 1 << 16 } else { 0 })
            | (if self.os { 1 << 17 } else { 0 })
            | (if self.edge { 1 << 18 } else { 0 })
            | (if self.pc { 1 << 19 } else { 0 })
            | (if self.int { 1 << 20 } else { 0 })
            | (if self.any_thread { 1 << 21 } else { 0 })
            | (if self.enable { 1 << 22 } else { 0 })
            | (if self.invert { 1 << 23 } else { 0 })
            | ((self.cmask as u64) << 24)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            event_select: (value & 0xFF) as u8,
            umask: ((value >> 8) & 0xFF) as u8,
            usr: (value & (1 << 16)) != 0,
            os: (value & (1 << 17)) != 0,
            edge: (value & (1 << 18)) != 0,
            pc: (value & (1 << 19)) != 0,
            int: (value & (1 << 20)) != 0,
            any_thread: (value & (1 << 21)) != 0,
            enable: (value & (1 << 22)) != 0,
            invert: (value & (1 << 23)) != 0,
            cmask: ((value >> 24) & 0xFF) as u8,
        }
    }
}

/// Fixed Counter Control Register layout
///
/// Controls the fixed-function performance counters.
///
/// Each counter uses 4 bits: [enable_os, enable_usr, any_thread, pmi]
#[derive(Debug, Clone, Copy, Default)]
pub struct FixedCtrCtrl {
    /// Fixed counter 0 controls (Instructions Retired)
    pub ctr0_os: bool,
    pub ctr0_usr: bool,
    pub ctr0_any_thread: bool,
    pub ctr0_pmi: bool,

    /// Fixed counter 1 controls (Core Cycles)
    pub ctr1_os: bool,
    pub ctr1_usr: bool,
    pub ctr1_any_thread: bool,
    pub ctr1_pmi: bool,

    /// Fixed counter 2 controls (Reference Cycles)
    pub ctr2_os: bool,
    pub ctr2_usr: bool,
    pub ctr2_any_thread: bool,
    pub ctr2_pmi: bool,
}

impl RegisterLayout for FixedCtrCtrl {
    fn to_msr_value(&self) -> u64 {
        let mut value = 0u64;

        // Counter 0 (bits 0-3)
        if self.ctr0_os {
            value |= 1 << 0;
        }
        if self.ctr0_usr {
            value |= 1 << 1;
        }
        if self.ctr0_any_thread {
            value |= 1 << 2;
        }
        if self.ctr0_pmi {
            value |= 1 << 3;
        }

        // Counter 1 (bits 4-7)
        if self.ctr1_os {
            value |= 1 << 4;
        }
        if self.ctr1_usr {
            value |= 1 << 5;
        }
        if self.ctr1_any_thread {
            value |= 1 << 6;
        }
        if self.ctr1_pmi {
            value |= 1 << 7;
        }

        // Counter 2 (bits 8-11)
        if self.ctr2_os {
            value |= 1 << 8;
        }
        if self.ctr2_usr {
            value |= 1 << 9;
        }
        if self.ctr2_any_thread {
            value |= 1 << 10;
        }
        if self.ctr2_pmi {
            value |= 1 << 11;
        }

        value
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            ctr0_os: (value & (1 << 0)) != 0,
            ctr0_usr: (value & (1 << 1)) != 0,
            ctr0_any_thread: (value & (1 << 2)) != 0,
            ctr0_pmi: (value & (1 << 3)) != 0,

            ctr1_os: (value & (1 << 4)) != 0,
            ctr1_usr: (value & (1 << 5)) != 0,
            ctr1_any_thread: (value & (1 << 6)) != 0,
            ctr1_pmi: (value & (1 << 7)) != 0,

            ctr2_os: (value & (1 << 8)) != 0,
            ctr2_usr: (value & (1 << 9)) != 0,
            ctr2_any_thread: (value & (1 << 10)) != 0,
            ctr2_pmi: (value & (1 << 11)) != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_perf_evtsel_round_trip() {
        let evtsel = CorePerfEvtSel {
            event_select: 0x3C,
            umask: 0x00,
            usr: true,
            os: true,
            enable: true,
            cmask: 0,
            ..Default::default()
        };

        let value = evtsel.to_msr_value();
        let decoded = CorePerfEvtSel::from_msr_value(value);

        assert_eq!(decoded.event_select, evtsel.event_select);
        assert_eq!(decoded.umask, evtsel.umask);
        assert_eq!(decoded.usr, evtsel.usr);
        assert_eq!(decoded.os, evtsel.os);
        assert_eq!(decoded.enable, evtsel.enable);
    }

    #[test]
    fn test_fixed_ctr_ctrl_round_trip() {
        let ctrl = FixedCtrCtrl {
            ctr0_os: true,
            ctr0_usr: true,
            ctr1_os: true,
            ctr1_usr: false,
            ctr2_os: false,
            ctr2_usr: true,
            ..Default::default()
        };

        let value = ctrl.to_msr_value();
        let decoded = FixedCtrCtrl::from_msr_value(value);

        assert_eq!(decoded.ctr0_os, ctrl.ctr0_os);
        assert_eq!(decoded.ctr0_usr, ctrl.ctr0_usr);
        assert_eq!(decoded.ctr1_os, ctrl.ctr1_os);
        assert_eq!(decoded.ctr2_usr, ctrl.ctr2_usr);
    }
}
