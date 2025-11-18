// PMU event definitions (architecture-aware)

use crate::common::CPU_ARCH;

#[derive(Debug, Clone, Copy)]
pub struct PmuEvent {
    pub event: u8,
    pub umask: u8,
    pub name: &'static str,
}

// Fixed counter events (architectural - same across all Intel CPUs)
pub const INST_RETIRED: &str = "InstructionsRetired";
pub const CPU_CLK_UNHALTED: &str = "UnhaltedCoreCycles";
pub const REF_CPU_CYCLES: &str = "UnhaltedReferenceCycles";

// Core events (common across architectures)
pub const COMMON_EVENTS: &[PmuEvent] = &[
    PmuEvent {
        event: 0x2E,
        umask: 0x4F,
        name: "LLCReference",
    },
    PmuEvent {
        event: 0x2E,
        umask: 0x41,
        name: "LLCMisses",
    },
    PmuEvent {
        event: 0x24,
        umask: 0x3F,
        name: "L2RequestMisses",
    },
    PmuEvent {
        event: 0x24,
        umask: 0xFF,
        name: "L2RequestReference",
    },
    PmuEvent {
        event: 0xF1,
        umask: 0x1F,
        name: "L2In",
    },
    PmuEvent {
        event: 0xF0,
        umask: 0x40,
        name: "L2Writeback",
    },
];

/// Get architecture-specific events
pub fn get_architecture_events() -> Vec<PmuEvent> {
    let mut events = COMMON_EVENTS.to_vec();

    // Add architecture-specific L2 prefetch events
    let prefetch_events = CPU_ARCH.l2_prefetch_events();
    for (event, umask, name) in prefetch_events {
        events.push(PmuEvent { event, umask, name });
    }

    // Add architecture-specific L2 eviction events
    let eviction_events = CPU_ARCH.l2_eviction_events();
    for (event, umask, name) in eviction_events {
        events.push(PmuEvent { event, umask, name });
    }

    events
}

/// Get a curated set of events for our 4 programmable counters
/// These are the most important metrics
pub fn get_default_event_set() -> Vec<PmuEvent> {
    vec![
        PmuEvent {
            event: 0x2E,
            umask: 0x4F,
            name: "LLCReference",
        },
        PmuEvent {
            event: 0x2E,
            umask: 0x41,
            name: "LLCMisses",
        },
        PmuEvent {
            event: 0x24,
            umask: 0x3F,
            name: "L2RequestMisses",
        },
        PmuEvent {
            event: 0x24,
            umask: 0xFF,
            name: "L2RequestReference",
        },
    ]
}

// MSR addresses for PMU
pub const IA32_PERF_GLOBAL_CTRL: u64 = 0x38F;
pub const IA32_FIXED_CTR_CTRL: u64 = 0x38D;
pub const IA32_PERF_GLOBAL_STATUS: u64 = 0x38E;
pub const IA32_PERF_GLOBAL_OVF_CTRL: u64 = 0x390;

// Fixed counters
pub const IA32_FIXED_CTR0: u64 = 0x309; // Instructions retired
pub const IA32_FIXED_CTR1: u64 = 0x30A; // Core cycles
pub const IA32_FIXED_CTR2: u64 = 0x30B; // Reference cycles

// Programmable counters
pub const IA32_PERFEVTSEL0: u64 = 0x186;
pub const IA32_PERFEVTSEL1: u64 = 0x187;
pub const IA32_PERFEVTSEL2: u64 = 0x188;
pub const IA32_PERFEVTSEL3: u64 = 0x189;

pub const IA32_PMC0: u64 = 0xC1;
pub const IA32_PMC1: u64 = 0xC2;
pub const IA32_PMC2: u64 = 0xC3;
pub const IA32_PMC3: u64 = 0xC4;

// TSC for time measurement
pub const IA32_TIME_STAMP_COUNTER: u64 = 0x10;

// Platform info for frequency
pub const MSR_PLATFORM_INFO: u64 = 0xCE;

impl PmuEvent {
    pub fn encode_for_perfevtsel(&self, user: bool, kernel: bool) -> u64 {
        let mut value = 0u64;
        value |= self.event as u64; // Event select [7:0]
        value |= (self.umask as u64) << 8; // Unit mask [15:8]
        value |= if user { 1 << 16 } else { 0 }; // USR [16]
        value |= if kernel { 1 << 17 } else { 0 }; // OS [17]
        value |= 1 << 22; // Enable [22]
        value
    }
}
