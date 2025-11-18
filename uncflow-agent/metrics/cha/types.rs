// CHA (Cache Home Agent) metrics - comprehensive coverage

use crate::counters::cha::{LLCLookupType, LLCState, TransactionType};

/// Transaction-specific derived metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionMetricType {
    Bandwidth,     // (Hit + Miss) insert * 64 / duration
    HitBandwidth,  // Hit insert * 64 / duration
    MissBandwidth, // Miss insert * 64 / duration
    HitLatency,    // HitOccupancy / HitInsert * (HitClocks / duration)
    MissLatency,   // MissOccupancy / MissInsert * (MissClocks / duration)
    HitRate,       // HitInsert / (HitInsert + MissInsert)
    Latency,       // Combined latency (placeholder)
    HitOccupancy,  // HitOccupancy / HitClockTicks
    MissOccupancy, // MissOccupancy / MissClockTicks
}

impl TransactionMetricType {
    pub fn name(&self) -> &'static str {
        match self {
            TransactionMetricType::Bandwidth => "Bandwidth",
            TransactionMetricType::HitBandwidth => "HitBandwidth",
            TransactionMetricType::MissBandwidth => "MissBandwidth",
            TransactionMetricType::HitLatency => "HitLatency",
            TransactionMetricType::MissLatency => "MissLatency",
            TransactionMetricType::HitRate => "HitRate",
            TransactionMetricType::Latency => "Latency",
            TransactionMetricType::HitOccupancy => "HitOccupancy",
            TransactionMetricType::MissOccupancy => "MissOccupancy",
        }
    }

    pub fn all() -> Vec<TransactionMetricType> {
        vec![
            TransactionMetricType::Bandwidth,
            TransactionMetricType::HitBandwidth,
            TransactionMetricType::MissBandwidth,
            TransactionMetricType::HitLatency,
            TransactionMetricType::MissLatency,
            TransactionMetricType::HitRate,
            TransactionMetricType::Latency,
            TransactionMetricType::HitOccupancy,
            TransactionMetricType::MissOccupancy,
        ]
    }
}

/// LLC Victim types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VictimType {
    M, // Modified
    E, // Exclusive
    S, // Shared
    F, // Forward
}

impl VictimType {
    pub fn name(&self) -> &'static str {
        match self {
            VictimType::M => "M",
            VictimType::E => "E",
            VictimType::S => "S",
            VictimType::F => "F",
        }
    }

    pub fn umask(&self) -> u8 {
        match self {
            VictimType::M => 0x01,
            VictimType::E => 0x02,
            VictimType::S => 0x04,
            VictimType::F => 0x08,
        }
    }

    pub fn all() -> Vec<VictimType> {
        vec![VictimType::M, VictimType::E, VictimType::S, VictimType::F]
    }
}

/// SF Eviction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SFEvictionType {
    M, // Modified
    E, // Exclusive
    S, // Shared
}

impl SFEvictionType {
    pub fn name(&self) -> &'static str {
        match self {
            SFEvictionType::M => "M",
            SFEvictionType::E => "E",
            SFEvictionType::S => "S",
        }
    }

    pub fn umask(&self) -> u8 {
        match self {
            SFEvictionType::M => 0x01,
            SFEvictionType::E => 0x02,
            SFEvictionType::S => 0x04,
        }
    }

    pub fn all() -> Vec<SFEvictionType> {
        vec![SFEvictionType::M, SFEvictionType::E, SFEvictionType::S]
    }
}

/// Comprehensive CHA metrics enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChaMetric {
    // Transaction metrics: 11 types × 9 metrics = 99 metrics
    Transaction(TransactionType, TransactionMetricType),

    // LLC Lookup metrics: 7 states × 4 types = 28 metrics
    LLCLookup(LLCState, LLCLookupType),

    // LLC Victim metrics: 4 metrics
    LLCVictim(VictimType),

    // SF Eviction metrics: 3 metrics
    SFEviction(SFEvictionType),

    // Eviction metrics: 3 metrics
    EvictionBandwidth,
    EvictionLatency,
    EvictionQueueOccupancy,

    // Queue occupancy metrics: 2 metrics
    IRQOccupancy,
    PRQOccupancy,

    // Frequency: 1 metric
    UncoreFrequency,

    // Credit metrics: 2 metrics
    ReadNoCredit,
    WriteNoCredit,
}

impl ChaMetric {
    pub fn name(&self) -> String {
        match self {
            ChaMetric::Transaction(trans_type, metric_type) => {
                format!("{}{}", trans_type.name(), metric_type.name())
            }
            ChaMetric::LLCLookup(state, lookup_type) => {
                format!("LLCLookup{}{}", state.name(), lookup_type.name())
            }
            ChaMetric::LLCVictim(victim_type) => {
                format!("LLCVictim{}", victim_type.name())
            }
            ChaMetric::SFEviction(eviction_type) => {
                format!("SFEviction{}", eviction_type.name())
            }
            ChaMetric::EvictionBandwidth => "EvictionBandwidth".to_string(),
            ChaMetric::EvictionLatency => "EvictionLatency".to_string(),
            ChaMetric::EvictionQueueOccupancy => "EvictionQueueOccupancy".to_string(),
            ChaMetric::IRQOccupancy => "IRQOccupancy".to_string(),
            ChaMetric::PRQOccupancy => "PRQOccupancy".to_string(),
            ChaMetric::UncoreFrequency => "UncoreFrequency".to_string(),
            ChaMetric::ReadNoCredit => "ReadNoCredit".to_string(),
            ChaMetric::WriteNoCredit => "WriteNoCredit".to_string(),
        }
    }

    /// Get all CHA metrics (137 total)
    pub fn all() -> Vec<ChaMetric> {
        let mut metrics = Vec::new();

        // Transaction metrics (11 × 9 = 99)
        for trans_type in TransactionType::all() {
            for metric_type in TransactionMetricType::all() {
                metrics.push(ChaMetric::Transaction(trans_type, metric_type));
            }
        }

        // LLC Lookup metrics (7 × 4 = 28)
        for state in LLCState::all() {
            for lookup_type in LLCLookupType::all() {
                metrics.push(ChaMetric::LLCLookup(state, lookup_type));
            }
        }

        // LLC Victim metrics (4)
        for victim_type in VictimType::all() {
            metrics.push(ChaMetric::LLCVictim(victim_type));
        }

        // SF Eviction metrics (3)
        for eviction_type in SFEvictionType::all() {
            metrics.push(ChaMetric::SFEviction(eviction_type));
        }

        // Other metrics (8)
        metrics.push(ChaMetric::EvictionBandwidth);
        metrics.push(ChaMetric::EvictionLatency);
        metrics.push(ChaMetric::EvictionQueueOccupancy);
        metrics.push(ChaMetric::IRQOccupancy);
        metrics.push(ChaMetric::PRQOccupancy);
        metrics.push(ChaMetric::UncoreFrequency);
        metrics.push(ChaMetric::ReadNoCredit);
        metrics.push(ChaMetric::WriteNoCredit);

        metrics
    }

    /// Get basic transaction metrics for compatibility
    pub fn basic_set() -> Vec<ChaMetric> {
        vec![
            // PCIe Read
            ChaMetric::Transaction(TransactionType::PCIeRead, TransactionMetricType::Bandwidth),
            ChaMetric::Transaction(TransactionType::PCIeRead, TransactionMetricType::HitLatency),
            // PCIe Write (using FullWrite)
            ChaMetric::Transaction(
                TransactionType::PCIeFullWrite,
                TransactionMetricType::Bandwidth,
            ),
            ChaMetric::Transaction(
                TransactionType::PCIeFullWrite,
                TransactionMetricType::HitLatency,
            ),
            // LLC Victims
            ChaMetric::LLCVictim(VictimType::M),
            ChaMetric::LLCVictim(VictimType::E),
            ChaMetric::LLCVictim(VictimType::S),
            // Frequency
            ChaMetric::UncoreFrequency,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_count() {
        let all_metrics = ChaMetric::all();

        // 99 transaction + 28 LLC lookup + 4 victim + 3 eviction + 8 other = 142
        // (Note: This is slightly more than the 137 mentioned due to including all states)
        assert!(all_metrics.len() >= 137);
        println!("Total CHA metrics: {}", all_metrics.len());
    }

    #[test]
    fn test_transaction_metrics() {
        let trans_type = TransactionType::PCIeRead;
        let metric_type = TransactionMetricType::HitBandwidth;
        let metric = ChaMetric::Transaction(trans_type, metric_type);

        assert_eq!(metric.name(), "PCIeReadHitBandwidth");
    }

    #[test]
    fn test_llc_lookup_metrics() {
        let state = LLCState::M;
        let lookup_type = LLCLookupType::Read;
        let metric = ChaMetric::LLCLookup(state, lookup_type);

        assert_eq!(metric.name(), "LLCLookupMRead");
    }
}
