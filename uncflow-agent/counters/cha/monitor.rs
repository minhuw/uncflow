// CHA (Cache Home Agent) monitoring with comprehensive event collection
// Supports event rotation for full transaction coverage

use crate::common::{arch::CPU_ARCH, msr};
use crate::counters::cha::ChaEventConfig;
use crate::error::Result;
use crate::metrics::cha::RawEventData;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// CHA MSR base addresses
const CHA_MSR_PMON_BOX_CTL: u64 = 0x0E00;
const CHA_MSR_PMON_CTL0: u64 = 0x0E01;
const CHA_MSR_PMON_CTR0: u64 = 0x0E08;
const CHA_MSR_PMON_BOX_FILTER0: u64 = 0x0E05;
const CHA_MSR_PMON_BOX_FILTER1: u64 = 0x0E06;

// CHA box stride (offset between CHA boxes)
const CHA_BOX_STRIDE: u64 = 0x10;

/// Event group for rotation scheduling
#[derive(Debug, Clone)]
struct EventGroup {
    name: String,
    config: ChaEventConfig,
    counter_configs: [(u8, u8); 4], // (event, umask) for 4 counters
}

impl EventGroup {
    fn from_config(config: ChaEventConfig) -> Self {
        let counter_configs = config.events;
        Self {
            name: config.name.clone(),
            config,
            counter_configs,
        }
    }
}

/// Event rotation scheduler
struct EventScheduler {
    groups: Vec<EventGroup>,
    current_index: usize,
    last_rotation: Instant,
    rotation_interval: Duration,
}

impl EventScheduler {
    fn new(rotation_interval: Duration) -> Self {
        Self {
            groups: Vec::new(),
            current_index: 0,
            last_rotation: Instant::now(),
            rotation_interval,
        }
    }

    fn add_event_group(&mut self, config: ChaEventConfig) {
        self.groups.push(EventGroup::from_config(config));
    }

    fn should_rotate(&self) -> bool {
        self.last_rotation.elapsed() >= self.rotation_interval
    }

    fn get_current_group(&self) -> Option<&EventGroup> {
        self.groups.get(self.current_index)
    }

    fn rotate(&mut self) {
        if self.groups.is_empty() {
            return;
        }

        self.current_index = (self.current_index + 1) % self.groups.len();
        self.last_rotation = Instant::now();
    }

    fn current_group_index(&self) -> usize {
        self.current_index
    }
}

/// Raw counter values for one CHA unit
#[derive(Debug, Clone, Default)]
struct ChaRawCounters {
    counter0: u64,
    counter1: u64,
    counter2: u64,
    counter3: u64,
}

/// CHA Monitor with comprehensive event collection
pub struct ChaMonitor {
    _socket: i32,
    cha_count: usize,
    representative_core: u32,

    // Event rotation
    scheduler: EventScheduler,

    // Previous counter values per CHA unit
    prev_counters: HashMap<usize, ChaRawCounters>,

    // Accumulated event data (aggregated across all CHA units)
    event_data: HashMap<String, RawEventData>,

    // Collection start time
    collection_start: Instant,
}

impl ChaMonitor {
    pub fn new(socket: i32) -> Result<Self> {
        let cha_count = CPU_ARCH.cha_count().unwrap_or(28) as usize;
        let representative_core = (socket * 28) as u32;

        tracing::info!(
            "Initializing comprehensive CHA monitor for socket {} with {} CHA boxes",
            socket,
            cha_count
        );

        // Event rotation every 2 seconds (allows time for counters to accumulate)
        let scheduler = EventScheduler::new(Duration::from_secs(2));

        Ok(Self {
            _socket: socket,
            cha_count,
            representative_core,
            scheduler,
            prev_counters: HashMap::new(),
            event_data: HashMap::new(),
            collection_start: Instant::now(),
        })
    }

    pub fn initialize(&mut self) -> Result<()> {
        // Setup event rotation with all transaction types
        self.setup_event_rotation();

        // Program initial event group
        if let Some(group) = self.scheduler.get_current_group() {
            for cha_id in 0..self.cha_count {
                self.program_event_group(cha_id, group)?;
            }
        }

        Ok(())
    }

    fn setup_event_rotation(&mut self) {
        // Add all transaction event groups (hit and miss)
        for config in ChaEventConfig::all_transactions() {
            self.scheduler.add_event_group(config);
        }

        tracing::info!(
            "Setup event rotation with {} groups (rotation every {:?})",
            self.scheduler.groups.len(),
            self.scheduler.rotation_interval
        );
    }

    fn program_event_group(&self, cha_id: usize, group: &EventGroup) -> Result<()> {
        let base_addr = CHA_MSR_PMON_BOX_CTL + (cha_id as u64 * CHA_BOX_STRIDE);
        let ctl_addr = base_addr;

        // Freeze the CHA box
        msr::Msr::instance().write(self.representative_core, ctl_addr, 0x00)?;

        // Setup filters if needed (for transaction opcodes)
        if group.config.opc0 != 0 {
            let filter0_addr = base_addr + (CHA_MSR_PMON_BOX_FILTER0 - CHA_MSR_PMON_BOX_CTL);
            let filter_value = group.config.opc0 as u64;
            msr::Msr::instance().write(self.representative_core, filter0_addr, filter_value)?;
        }

        if group.config.state != 0 {
            let filter1_addr = base_addr + (CHA_MSR_PMON_BOX_FILTER1 - CHA_MSR_PMON_BOX_CTL);
            let filter_value = (group.config.state as u64) << 17; // State field at bits 17-23
            msr::Msr::instance().write(self.representative_core, filter1_addr, filter_value)?;
        }

        // Program all 4 counters
        let ctl0_addr = base_addr + (CHA_MSR_PMON_CTL0 - CHA_MSR_PMON_BOX_CTL);
        for i in 0..4 {
            let (event, umask) = group.counter_configs[i];
            if event != 0 || umask != 0 {
                let ctl_addr = ctl0_addr + i as u64;
                let event_select = (event as u64) | ((umask as u64) << 8) | (1 << 22); // Enable bit
                msr::Msr::instance().write(self.representative_core, ctl_addr, event_select)?;
            }
        }

        // Unfreeze the CHA box
        msr::Msr::instance().write(self.representative_core, ctl_addr, 0x10000)?;

        Ok(())
    }

    fn read_cha_counters(&self, cha_id: usize) -> Result<ChaRawCounters> {
        let base_addr = CHA_MSR_PMON_CTR0 + (cha_id as u64 * CHA_BOX_STRIDE);

        Ok(ChaRawCounters {
            counter0: msr::Msr::instance().read(self.representative_core, base_addr)?,
            counter1: msr::Msr::instance().read(self.representative_core, base_addr + 1)?,
            counter2: msr::Msr::instance().read(self.representative_core, base_addr + 2)?,
            counter3: msr::Msr::instance().read(self.representative_core, base_addr + 3)?,
        })
    }

    fn collect_current_event_group(&mut self) -> Result<()> {
        let group = match self.scheduler.get_current_group() {
            Some(g) => g,
            None => return Ok(()),
        };

        let mut aggregated = [0u64; 4];
        let duration = self.collection_start.elapsed();

        // Aggregate counters across all CHA units
        for cha_id in 0..self.cha_count {
            let current = self.read_cha_counters(cha_id)?;
            let prev = self.prev_counters.get(&cha_id).cloned().unwrap_or_default();

            // Calculate deltas
            aggregated[0] += current.counter0.saturating_sub(prev.counter0);
            aggregated[1] += current.counter1.saturating_sub(prev.counter1);
            aggregated[2] += current.counter2.saturating_sub(prev.counter2);
            aggregated[3] += current.counter3.saturating_sub(prev.counter3);

            // Save for next iteration
            self.prev_counters.insert(cha_id, current);
        }

        // Store the aggregated data
        let event_name = &group.name;
        let data = RawEventData {
            occupancy: aggregated[0],
            insert: aggregated[1],
            clockticks: aggregated[2],
            duration,
        };

        // Accumulate with existing data (for this event group)
        self.event_data
            .entry(event_name.clone())
            .and_modify(|e| {
                e.occupancy += data.occupancy;
                e.insert += data.insert;
                e.clockticks += data.clockticks;
                e.duration = duration;
            })
            .or_insert(data);

        Ok(())
    }

    pub fn collect(&mut self) -> Result<HashMap<String, RawEventData>> {
        // Collect data from current event group
        self.collect_current_event_group()?;

        // Check if it's time to rotate
        if self.scheduler.should_rotate() {
            self.scheduler.rotate();
            let current_idx = self.scheduler.current_group_index();

            if let Some(next_group) = self.scheduler.groups.get(current_idx) {
                tracing::debug!(
                    "Rotating to event group: {} ({}/{})",
                    next_group.name,
                    current_idx + 1,
                    self.scheduler.groups.len()
                );

                // Program all CHA units with the new event group
                for cha_id in 0..self.cha_count {
                    self.program_event_group(cha_id, next_group)?;
                }

                // Reset previous counters for clean delta calculation
                self.prev_counters.clear();
            }
        }

        // Return a clone of accumulated event data
        Ok(self.event_data.clone())
    }

    /// Get event data for calculator
    pub fn get_event_data(&self) -> &HashMap<String, RawEventData> {
        &self.event_data
    }

    /// Reset accumulated data (e.g., after exporting)
    pub fn reset_event_data(&mut self) {
        // Don't clear completely, just reset durations
        // This allows continuous accumulation between rotations
        for data in self.event_data.values_mut() {
            data.duration = Duration::from_secs(0);
        }
        self.collection_start = Instant::now();
    }
}

// Legacy compatibility structure
#[derive(Debug, Clone, Default)]
pub struct ChaMetrics {
    pub llc_lookup_read: u64,
    pub llc_lookup_write: u64,
    pub llc_miss_read: u64,
    pub llc_miss_write: u64,
    pub llc_victim_m: u64,
    pub llc_victim_e: u64,
    pub llc_victim_s: u64,
    pub tor_occupancy: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::counters::cha::TransactionType;

    #[test]
    fn test_event_scheduler() {
        let mut scheduler = EventScheduler::new(Duration::from_secs(1));

        // Add some test events
        let config1 = ChaEventConfig::transaction(TransactionType::PCIeRead, true);
        let config2 = ChaEventConfig::transaction(TransactionType::PCIeRead, false);

        scheduler.add_event_group(config1);
        scheduler.add_event_group(config2);

        assert_eq!(scheduler.groups.len(), 2);
        assert_eq!(scheduler.current_index, 0);

        // Rotate
        scheduler.rotate();
        assert_eq!(scheduler.current_group_index(), 1);

        scheduler.rotate();
        assert_eq!(scheduler.current_group_index(), 0); // Wrap around
    }

    #[test]
    fn test_event_group_count() {
        let configs = ChaEventConfig::all_transactions();
        // Should have 11 transaction types × 2 (hit/miss) = 22 groups
        assert_eq!(configs.len(), 22);
    }
}
