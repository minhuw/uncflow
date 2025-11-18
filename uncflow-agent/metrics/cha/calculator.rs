// CHA Metric Calculator - Derives metrics from basic events

use std::collections::HashMap;
use std::time::Duration;

use crate::counters::cha::{LLCLookupType, LLCState, TransactionType};
use crate::metrics::cha::TransactionMetricType;

const CACHELINE_SIZE: u64 = 64;

/// Raw event data from hardware counters
#[derive(Debug, Clone, Default)]
pub struct RawEventData {
    pub occupancy: u64,
    pub insert: u64,
    pub clockticks: u64,
    pub duration: Duration,
}

/// Calculator for derived CHA metrics
pub struct MetricCalculator {
    /// Basic events keyed by event name (e.g., "PCIe Read Hit", "PCIe Read Miss")
    pub events: HashMap<String, RawEventData>,
}

impl MetricCalculator {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
        }
    }

    /// Store a basic event measurement
    pub fn store_event(&mut self, name: String, data: RawEventData) {
        self.events.insert(name, data);
    }

    /// Calculate bandwidth in GB/s from insert count
    fn calculate_bandwidth(insert: u64, duration: Duration) -> f64 {
        let seconds = duration.as_secs_f64();
        if seconds == 0.0 {
            return 0.0;
        }
        (insert as f64 * CACHELINE_SIZE as f64) / seconds / 1e9
    }

    /// Calculate latency in nanoseconds
    fn calculate_latency(occupancy: u64, insert: u64, clockticks: u64, duration: Duration) -> f64 {
        if insert == 0 || clockticks == 0 {
            return 0.0;
        }

        let elapsed_ns = duration.as_nanos() as f64;
        (occupancy as f64 / insert as f64) * (clockticks as f64 / elapsed_ns)
    }

    /// Calculate hit rate as ratio
    fn calculate_hit_rate(hit_insert: u64, miss_insert: u64) -> f64 {
        let total = hit_insert + miss_insert;
        if total == 0 {
            return 0.0;
        }
        hit_insert as f64 / total as f64
    }

    /// Calculate occupancy ratio
    fn calculate_occupancy(occupancy: u64, clockticks: u64) -> f64 {
        if clockticks == 0 {
            return 0.0;
        }
        occupancy as f64 / clockticks as f64
    }

    /// Calculate all transaction metrics for a given transaction type
    pub fn calculate_transaction_metrics(
        &self,
        trans_type: TransactionType,
    ) -> HashMap<TransactionMetricType, f64> {
        let mut metrics = HashMap::new();

        let hit_name = format!("{} Hit", trans_type.name());
        let miss_name = format!("{} Miss", trans_type.name());

        let hit_data = self.events.get(&hit_name);
        let miss_data = self.events.get(&miss_name);

        if let (Some(hit), Some(miss)) = (hit_data, miss_data) {
            // Bandwidth metrics
            let hit_bw = Self::calculate_bandwidth(hit.insert, hit.duration);
            let miss_bw = Self::calculate_bandwidth(miss.insert, miss.duration);
            let total_bw = hit_bw + miss_bw;

            metrics.insert(TransactionMetricType::Bandwidth, total_bw);
            metrics.insert(TransactionMetricType::HitBandwidth, hit_bw);
            metrics.insert(TransactionMetricType::MissBandwidth, miss_bw);

            // Latency metrics
            let hit_lat =
                Self::calculate_latency(hit.occupancy, hit.insert, hit.clockticks, hit.duration);
            let miss_lat = Self::calculate_latency(
                miss.occupancy,
                miss.insert,
                miss.clockticks,
                miss.duration,
            );

            metrics.insert(TransactionMetricType::HitLatency, hit_lat);
            metrics.insert(TransactionMetricType::MissLatency, miss_lat);
            metrics.insert(TransactionMetricType::Latency, 0.0); // Placeholder

            // Hit rate
            let hit_rate = Self::calculate_hit_rate(hit.insert, miss.insert);
            metrics.insert(TransactionMetricType::HitRate, hit_rate);

            // Occupancy ratios
            let hit_occ = Self::calculate_occupancy(hit.occupancy, hit.clockticks);
            let miss_occ = Self::calculate_occupancy(miss.occupancy, miss.clockticks);

            metrics.insert(TransactionMetricType::HitOccupancy, hit_occ);
            metrics.insert(TransactionMetricType::MissOccupancy, miss_occ);
        }

        metrics
    }

    /// Get LLC lookup metric value
    pub fn get_llc_lookup(&self, state: LLCState, lookup_type: LLCLookupType) -> u64 {
        let name = format!("LLC Lookup {} {}", state.name(), lookup_type.name());
        self.events.get(&name).map(|data| data.insert).unwrap_or(0)
    }

    /// Get LLC victim count
    pub fn get_llc_victim(&self, victim_type: &str) -> u64 {
        let name = format!("LLC Victim {victim_type}");
        self.events.get(&name).map(|data| data.insert).unwrap_or(0)
    }

    /// Get SF eviction count
    pub fn get_sf_eviction(&self, eviction_type: &str) -> u64 {
        let name = format!("SF Eviction {eviction_type}");
        self.events.get(&name).map(|data| data.insert).unwrap_or(0)
    }

    /// Calculate eviction bandwidth
    pub fn calculate_eviction_bandwidth(&self) -> f64 {
        if let Some(data) = self.events.get("Eviction") {
            Self::calculate_bandwidth(data.insert, data.duration)
        } else {
            0.0
        }
    }

    /// Calculate eviction latency
    pub fn calculate_eviction_latency(&self) -> f64 {
        if let Some(data) = self.events.get("Eviction") {
            Self::calculate_latency(data.occupancy, data.insert, data.clockticks, data.duration)
        } else {
            0.0
        }
    }

    /// Calculate eviction queue occupancy
    pub fn calculate_eviction_queue_occupancy(&self) -> f64 {
        if let Some(data) = self.events.get("Eviction") {
            Self::calculate_occupancy(data.occupancy, data.clockticks)
        } else {
            0.0
        }
    }

    /// Calculate uncore frequency in GHz
    pub fn calculate_uncore_frequency(&self) -> f64 {
        // Use clockticks from any event to calculate frequency
        for data in self.events.values() {
            if data.clockticks > 0 && data.duration.as_secs_f64() > 0.0 {
                return data.clockticks as f64 / data.duration.as_secs_f64() / 1e9;
            }
        }
        0.0
    }

    /// Get queue occupancy metric
    pub fn get_queue_occupancy(&self, queue_name: &str) -> f64 {
        if let Some(data) = self.events.get(queue_name) {
            Self::calculate_occupancy(data.occupancy, data.clockticks)
        } else {
            0.0
        }
    }

    /// Get credit metric
    pub fn get_credit_metric(&self, metric_name: &str) -> u64 {
        self.events
            .get(metric_name)
            .map(|data| data.insert)
            .unwrap_or(0)
    }
}

impl Default for MetricCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_calculation() {
        let bw = MetricCalculator::calculate_bandwidth(1000, Duration::from_secs(1));
        // 1000 * 64 bytes / 1 second = 64000 bytes/s = 0.000064 GB/s
        assert!((bw - 0.000_064).abs() < 1e-9);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let hit_rate = MetricCalculator::calculate_hit_rate(800, 200);
        assert!((hit_rate - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_occupancy_calculation() {
        let occ = MetricCalculator::calculate_occupancy(1000, 10000);
        assert!((occ - 0.1).abs() < 1e-9);
    }
}
