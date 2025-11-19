// IIO (Integrated IO) Monitor
//
// Now uses uncflow-raw for type-safe hardware register programming

use crate::common::msr;
use crate::error::Result;
use crate::metrics::iio::IioMetric;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Import hardware definitions from uncflow-raw
use uncflow_raw::current_arch::iio::{self, IioCounterControl};
use uncflow_raw::RegisterLayout;

// Business logic constants
const CACHELINE_SIZE: u64 = 64;

// IIO Event configurations
#[derive(Debug, Clone)]
struct IioEventConfig {
    name: &'static str,
    events: [(u8, u8, u8, u8); 4], // (event, umask, ch_mask, fc_mask)
}

const IIO_EVENTS: &[IioEventConfig] = &[
    IioEventConfig {
        name: "TLB_Miss_Group",
        events: [
            (0x41, 0x20, 0xFF, 0x07), // IIO TLB Miss
            (0x41, 0x04, 0xFF, 0x07), // IIO L1 Miss
            (0x41, 0x08, 0xFF, 0x07), // IIO L2 Miss
            (0x41, 0x10, 0xFF, 0x07), // IIO L3 Miss
        ],
    },
    IioEventConfig {
        name: "TLB_Hit_Group",
        events: [
            (0x41, 0x01, 0xFF, 0x07), // IIO TLB Hit
            (0x41, 0x02, 0xFF, 0x07), // IIO Context Miss
            (0x41, 0x40, 0xFF, 0x07), // IIO TLB Full
            (0x41, 0x80, 0xFF, 0x07), // IIO TLB1 Miss
        ],
    },
    IioEventConfig {
        name: "Occupancy_Group",
        events: [
            (0x40, 0x00, 0xFF, 0x07), // IIO Occupancy
            (0xC2, 0x04, 0xFF, 0x07), // IIO Comp Inserts
            (0xD5, 0x00, 0xFF, 0x07), // IIO Comp Occupancy
            (0x01, 0x00, 0xFF, 0x07), // Clockticks
        ],
    },
];

#[derive(Debug)]
struct IioCounterUnit {
    core: u32,
    index: usize,
}

impl IioCounterUnit {
    fn new(core: u32, index: usize) -> Result<Self> {
        Ok(Self { core, index })
    }

    fn freeze_and_reset(&self) -> Result<()> {
        let ctrl_addr = iio::msr::IIO_UNIT_BOX_CTL[self.index];
        msr::write(self.core, ctrl_addr, 0x100)?; // Freeze
        msr::write(self.core, ctrl_addr, 0x102)?; // Reset
        Ok(())
    }

    fn unfreeze(&self) -> Result<()> {
        let ctrl_addr = iio::msr::IIO_UNIT_BOX_CTL[self.index];
        msr::write(self.core, ctrl_addr, 0)?;
        Ok(())
    }

    fn program(&self, config: &IioEventConfig) -> Result<()> {
        // Try to freeze and reset, but don't fail if it doesn't work
        // Some systems may have read-only IIO MSRs
        if let Err(e) = self.freeze_and_reset() {
            tracing::debug!(
                "IIO freeze/reset not supported on this system (unit {}): {}",
                self.index,
                e
            );
            return Err(e);
        }

        let ctrl_addrs = [
            iio::msr::IIO_UNIT_CTL0[self.index],
            iio::msr::IIO_UNIT_CTL1[self.index],
            iio::msr::IIO_UNIT_CTL2[self.index],
            iio::msr::IIO_UNIT_CTL3[self.index],
        ];

        for (i, &(event, umask, ch_mask, fc_mask)) in config.events.iter().enumerate() {
            // Use type-safe register struct from uncflow-raw
            let ctrl = IioCounterControl {
                event_select: event,
                unit_mask: umask,
                reset_counter: true,
                overflow_enable: true,
                enable: true,
                channel_mask: ch_mask,
                fc_mask,
                ..Default::default()
            };

            // Validate before writing (type safety!)
            ctrl.validate()
                .map_err(|e| crate::error::UncflowError::HardwareError(e.to_string()))?;

            // Convert to MSR value and write
            // If write fails, propagate error
            msr::write(self.core, ctrl_addrs[i], ctrl.to_msr_value())?;
        }

        self.unfreeze()?;
        Ok(())
    }

    fn read_counters(&self) -> Result<[u64; 5]> {
        let ctr_addrs = [
            iio::msr::IIO_UNIT_CTR0[self.index],
            iio::msr::IIO_UNIT_CTR1[self.index],
            iio::msr::IIO_UNIT_CTR2[self.index],
            iio::msr::IIO_UNIT_CTR3[self.index],
            iio::msr::IIO_UNIT_CLK[self.index],
        ];

        let mut values = [0u64; 5];
        let mask = (1u64 << iio::UNCORE_COUNTER_WIDTH_BITS) - 1;
        for (i, &addr) in ctr_addrs.iter().enumerate() {
            values[i] = msr::read(self.core, addr)? & mask;
        }

        Ok(values)
    }
}

#[derive(Debug)]
pub struct IioMonitor {
    socket: i32,
    core: u32,
    units: Vec<IioCounterUnit>,
    event_results: HashMap<String, Vec<[u64; 5]>>,
    pcie_last_values: Option<[[u64; iio::IIO_PCIE_PORT_COUNT * 2]; iio::IIO_CHANNEL_COUNT]>,
    pcie_last_time: Option<Instant>,
    programmable_warned: bool, // Track if we've already warned about programmable counters
}

impl IioMonitor {
    pub fn new(socket: i32) -> Result<Self> {
        let core = (socket as u32) * 16;

        let mut units = Vec::new();
        for i in 0..iio::IIO_CHANNEL_COUNT {
            units.push(IioCounterUnit::new(core, i)?);
        }

        Ok(Self {
            socket,
            core,
            units,
            event_results: HashMap::new(),
            pcie_last_values: None,
            pcie_last_time: None,
            programmable_warned: false,
        })
    }

    pub fn collect_metrics(&mut self) -> Result<HashMap<IioMetric, f64>> {
        let mut metrics = HashMap::new();

        // Try to collect programmable counter metrics
        // If this fails (MSR writes not supported), we'll only collect PCIe bandwidth
        let programmable_supported = self.try_collect_programmable_metrics(&mut metrics);

        if !programmable_supported && !self.programmable_warned {
            tracing::warn!(
                "IIO programmable counters not available on socket {} (MSR writes protected). \
                 Only PCIe bandwidth metrics will be reported.",
                self.socket
            );
            self.programmable_warned = true; // Only warn once
        }

        // Collect PCIe free-running counter metrics (these are always read-only)
        self.collect_pcie_bandwidth(&mut metrics)?;

        Ok(metrics)
    }

    fn try_collect_programmable_metrics(&mut self, metrics: &mut HashMap<IioMetric, f64>) -> bool {
        // Try to collect programmable counter metrics
        for event_config in IIO_EVENTS {
            // Try to program all units for this event
            let mut program_failed = false;
            for unit in &self.units {
                if let Err(e) = unit.program(event_config) {
                    tracing::debug!("Failed to program IIO unit: {}", e);
                    program_failed = true;
                    break;
                }
            }

            if program_failed {
                // MSR writes not supported - return false
                return false;
            }

            // Sleep to collect data
            std::thread::sleep(Duration::from_secs(1));

            // Read counters
            let mut all_values = Vec::new();
            for unit in &self.units {
                match unit.read_counters() {
                    Ok(values) => all_values.push(values),
                    Err(e) => {
                        tracing::debug!("Failed to read IIO counters: {}", e);
                        return false;
                    }
                }
            }

            self.event_results
                .insert(event_config.name.to_string(), all_values);
        }

        // Calculate metrics from programmable counters
        if let Err(e) = self.calculate_programmable_metrics(metrics) {
            tracing::debug!("Failed to calculate programmable metrics: {}", e);
            return false;
        }

        true
    }

    fn calculate_programmable_metrics(&self, metrics: &mut HashMap<IioMetric, f64>) -> Result<()> {
        // TLB Miss Group
        if let Some(values) = self.event_results.get("TLB_Miss_Group") {
            let tlb_miss: u64 = values.iter().map(|v| v[0]).sum();
            let l1_miss: u64 = values.iter().map(|v| v[1]).sum();
            let l2_miss: u64 = values.iter().map(|v| v[2]).sum();
            let l3_miss: u64 = values.iter().map(|v| v[3]).sum();

            metrics.insert(IioMetric::IIOTLBMiss, tlb_miss as f64);
            metrics.insert(IioMetric::IIOL1Miss, l1_miss as f64);
            metrics.insert(IioMetric::IIOL2Miss, l2_miss as f64);
            metrics.insert(IioMetric::IIOL3Miss, l3_miss as f64);
        }

        // TLB Hit Group
        if let Some(values) = self.event_results.get("TLB_Hit_Group") {
            let tlb_hit: u64 = values.iter().map(|v| v[0]).sum();
            let context_miss: u64 = values.iter().map(|v| v[1]).sum();
            let tlb_full: u64 = values.iter().map(|v| v[2]).sum();
            let tlb1_miss: u64 = values.iter().map(|v| v[3]).sum();

            metrics.insert(IioMetric::IIOTLBHit, tlb_hit as f64);
            metrics.insert(IioMetric::IIOContextMiss, context_miss as f64);
            metrics.insert(IioMetric::IIOTLBFull, tlb_full as f64);
            metrics.insert(IioMetric::IIOTLB1Miss, tlb1_miss as f64);
        }

        // Occupancy Group
        if let Some(values) = self.event_results.get("Occupancy_Group") {
            let occupancy: u64 = values.iter().map(|v| v[0]).sum();
            let clockticks: u64 = values.iter().map(|v| v[3]).sum();

            if clockticks > 0 {
                let frequency = clockticks as f64 / 1e9; // GHz
                metrics.insert(IioMetric::IIOFrequency, frequency);

                let normalized_occupancy = occupancy as f64 / clockticks as f64;
                metrics.insert(IioMetric::IIOOccupancy, normalized_occupancy);
            }
        }

        Ok(())
    }

    fn collect_pcie_bandwidth(&mut self, metrics: &mut HashMap<IioMetric, f64>) -> Result<()> {
        let mut current_values = [[0u64; iio::IIO_PCIE_PORT_COUNT * 2]; iio::IIO_CHANNEL_COUNT];

        // Read all PCIe counters
        #[allow(clippy::needless_range_loop)]
        for ch in 0..iio::IIO_CHANNEL_COUNT {
            for port in 0..iio::IIO_PCIE_PORT_COUNT {
                let in_addr = iio::msr::IIO_PCIE_BANDWIDTH_IN[ch][port];
                let out_addr = iio::msr::IIO_PCIE_BANDWIDTH_OUT[ch][port];

                let in_val =
                    msr::read(self.core, in_addr)? & ((1u64 << iio::IIO_COUNTER_WIDTH_BITS) - 1);
                let out_val =
                    msr::read(self.core, out_addr)? & ((1u64 << iio::IIO_COUNTER_WIDTH_BITS) - 1);

                current_values[ch][port] = in_val;
                current_values[ch][port + iio::IIO_PCIE_PORT_COUNT] = out_val;
            }
        }

        let current_time = Instant::now();

        // Calculate bandwidth if we have previous values
        if let (Some(last_values), Some(last_time)) = (&self.pcie_last_values, self.pcie_last_time)
        {
            let elapsed = current_time.duration_since(last_time).as_secs_f64();

            for ch in 0..iio::IIO_CHANNEL_COUNT {
                for port in 0..iio::IIO_PCIE_PORT_COUNT {
                    // IN bandwidth
                    let in_delta = if current_values[ch][port] >= last_values[ch][port] {
                        current_values[ch][port] - last_values[ch][port]
                    } else {
                        (1u64 << iio::IIO_COUNTER_WIDTH_BITS) - last_values[ch][port]
                            + current_values[ch][port]
                    };
                    let in_bandwidth = (in_delta as f64 * CACHELINE_SIZE as f64) / elapsed / 1e9;
                    metrics.insert(IioMetric::PCIeInBandwidth(ch, port), in_bandwidth);

                    // OUT bandwidth
                    let out_idx = port + iio::IIO_PCIE_PORT_COUNT;
                    let out_delta = if current_values[ch][out_idx] >= last_values[ch][out_idx] {
                        current_values[ch][out_idx] - last_values[ch][out_idx]
                    } else {
                        (1u64 << iio::IIO_COUNTER_WIDTH_BITS) - last_values[ch][out_idx]
                            + current_values[ch][out_idx]
                    };
                    let out_bandwidth = (out_delta as f64 * CACHELINE_SIZE as f64) / elapsed / 1e9;
                    metrics.insert(IioMetric::PCIeOutBandwidth(ch, port), out_bandwidth);
                }
            }
        }

        self.pcie_last_values = Some(current_values);
        self.pcie_last_time = Some(current_time);

        Ok(())
    }

    pub fn socket(&self) -> i32 {
        self.socket
    }
}
