// CHA Event Configurations for Skylake-SP
use crate::{enum_with_data, enum_with_opcodes};

// Transaction types for CHA cache transaction monitoring
enum_with_opcodes! {
    pub enum TransactionType {
        PCIeRead => ("PCIeRead", 0x21E, 0),
        PCIeFullWrite => ("PCIeFullWrite", 0x248, 0),
        PCIePartialWrite => ("PCIePartialWrite", 0x249, 0),
        PCIeWriteBack => ("PCIeWriteBack", 0x194, 0),
        DRDRead => ("DRDRead", 0x202, 0),  // Core demand read
        RFO => ("RFO", 0x200, 0),           // Read-for-ownership
        ItoM => ("ItoM", 0x204, 0),         // Invalid-to-modified
        CLFlush => ("CLFlush", 0x204, 0),   // Cache line flush (same as ItoM)
        WbMtoI => ("WbMtoI", 0x1C4, 0),     // Writeback modified-to-invalid
        RxCIRQ => ("RxCIRQ", 0x180, 0),     // RxC IRQ
        RxCPRQ => ("RxCPRQ", 0x181, 0),     // RxC PRQ
    }
}

// LLC cache line states
enum_with_data! {
    pub enum LLCState: u32 {
        M => ("M", 0x40),       // Modified
        E => ("E", 0x20),       // Exclusive
        S => ("S", 0x02),       // Shared
        I => ("I", 0x01),       // Invalid
        SFM => ("SFM", 0x08),   // Snoop Filter Modified
        SFE => ("SFE", 0x04),   // Snoop Filter Exclusive
        SFS => ("SFS", 0x02),   // Snoop Filter Shared
    }
    impl state_value -> u32
}

// LLC lookup types
enum_with_data! {
    pub enum LLCLookupType: u8 {
        Read => ("Read", 0x03),
        Write => ("Write", 0x05),
        RemoteSnoop => ("RemoteSnoop", 0x09),
        Any => ("Any", 0x11),
    }
    impl umask -> u8
}

/// Basic event types for cache transaction monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicEventType {
    Occupancy,
    Insert,
    ClockTicks,
}

impl BasicEventType {
    pub fn name(&self) -> &'static str {
        match self {
            BasicEventType::Occupancy => "Occupancy",
            BasicEventType::Insert => "Insert",
            BasicEventType::ClockTicks => "ClockTicks",
        }
    }

    /// Get event code for Skylake-SP
    pub fn event_code(&self) -> u8 {
        match self {
            BasicEventType::Occupancy => 0x36,
            BasicEventType::Insert => 0x35,
            BasicEventType::ClockTicks => 0x00,
        }
    }

    /// Get umask based on hit/miss
    pub fn umask(&self, is_hit: bool) -> u8 {
        match (self, is_hit) {
            (BasicEventType::Occupancy, true) => 0x14,  // IO_HIT
            (BasicEventType::Occupancy, false) => 0x24, // IO_MISS
            (BasicEventType::Insert, true) => 0x14,
            (BasicEventType::Insert, false) => 0x24,
            (BasicEventType::ClockTicks, _) => 0x00,
        }
    }
}

/// CHA event configuration
#[derive(Debug, Clone)]
pub struct ChaEventConfig {
    pub name: String,
    pub transaction_type: Option<TransactionType>,
    pub is_hit: Option<bool>,
    pub events: [(u8, u8); 4], // (event, umask) pairs for 4 counters
    pub opc0: u32,
    pub opc1: u32,
    pub state: u32,
}

impl ChaEventConfig {
    /// Create a transaction hit/miss event config
    pub fn transaction(trans_type: TransactionType, is_hit: bool) -> Self {
        let (opc0, opc1) = trans_type.opcodes();
        let name = format!(
            "{} {}",
            trans_type.name(),
            if is_hit { "Hit" } else { "Miss" }
        );

        let events = [
            (
                BasicEventType::Occupancy.event_code(),
                BasicEventType::Occupancy.umask(is_hit),
            ),
            (
                BasicEventType::Insert.event_code(),
                BasicEventType::Insert.umask(is_hit),
            ),
            (BasicEventType::ClockTicks.event_code(), 0),
            (0, 0), // Unused counter
        ];

        Self {
            name,
            transaction_type: Some(trans_type),
            is_hit: Some(is_hit),
            events,
            opc0,
            opc1,
            state: 0,
        }
    }

    /// Create LLC lookup event config
    pub fn llc_lookup(state: LLCState, lookup_type: LLCLookupType) -> Self {
        let name = format!("LLC Lookup {} {}", state.name(), lookup_type.name());
        let events = [
            (0x34, lookup_type.umask()),
            (0x00, 0), // Placeholder
            (0x00, 0),
            (0x00, 0),
        ];

        Self {
            name,
            transaction_type: None,
            is_hit: None,
            events,
            opc0: 0,
            opc1: 0,
            state: state.state_value(),
        }
    }

    /// Create eviction event config
    pub fn eviction() -> Self {
        Self {
            name: "Eviction".to_string(),
            transaction_type: None,
            is_hit: None,
            events: [
                (0x36, 0x32), // Occupancy
                (0x35, 0x32), // Insert
                (0x00, 0x00), // ClockTicks
                (0x00, 0x00),
            ],
            opc0: 0,
            opc1: 0,
            state: 0,
        }
    }

    /// Generate all transaction event configs (22 total: 11 types × 2 hit/miss)
    pub fn all_transactions() -> Vec<Self> {
        let mut configs = Vec::new();
        for trans_type in TransactionType::all() {
            configs.push(Self::transaction(trans_type, true)); // Hit
            configs.push(Self::transaction(trans_type, false)); // Miss
        }
        configs
    }

    /// Generate all LLC lookup configs (28 total: 7 states × 4 types)
    pub fn all_llc_lookups() -> Vec<Self> {
        let mut configs = Vec::new();
        for state in LLCState::all() {
            for lookup_type in LLCLookupType::all() {
                configs.push(Self::llc_lookup(state, lookup_type));
            }
        }
        configs
    }
}
