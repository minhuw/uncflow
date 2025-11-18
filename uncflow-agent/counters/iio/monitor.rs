// IIO (Integrated IO) Monitor

use crate::common::msr;
use crate::error::Result;
use crate::metrics::iio::IioMetric;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Skylake IIO MSR addresses (3 IIO units per socket)
const IIO_CHANNEL: usize = 3;
const IIO_PCIE_PORT_NUM: usize = 4;
const _IIO_COUNTERS: usize = 4;
const IIO_COUNTER_WIDTH: u64 = 36;
const UNCORE_COUNTER_WIDTH: u64 = 48;
const CACHELINE_SIZE: u64 = 64;

const IIO_UNIT_BOX_CTL: [u64; 3] = [0x0A60, 0x0A80, 0x0AA0];
const _IIO_UNIT_BOX_STATUS: [u64; 3] = [0x0A67, 0x0A87, 0x0AA7];

const IIO_UNIT_CTL0: [u64; 3] = [0x0A68, 0x0A88, 0x0AA8];
const IIO_UNIT_CTL1: [u64; 3] = [0x0A69, 0x0A89, 0x0AA9];
const IIO_UNIT_CTL2: [u64; 3] = [0x0A6A, 0x0A8A, 0x0AAA];
const IIO_UNIT_CTL3: [u64; 3] = [0x0A6B, 0x0A8B, 0x0AAB];

const IIO_UNIT_CTR0: [u64; 3] = [0x0A61, 0x0A81, 0x0AA1];
const IIO_UNIT_CTR1: [u64; 3] = [0x0A62, 0x0A82, 0x0AA2];
const IIO_UNIT_CTR2: [u64; 3] = [0x0A63, 0x0A83, 0x0AA3];
const IIO_UNIT_CTR3: [u64; 3] = [0x0A64, 0x0A84, 0x0AA4];
const IIO_UNIT_CLK: [u64; 3] = [0x0A65, 0x0A85, 0x0AA5];

// PCIe free-running bandwidth counters
const IIO_PCIE_BANDWIDTH_IN: [[u64; 4]; 3] = [
    [0x0B10, 0x0B11, 0x0B12, 0x0B13],
    [0x0B20, 0x0B21, 0x0B22, 0x0B23],
    [0x0B30, 0x0B31, 0x0B32, 0x0B33],
];

const IIO_PCIE_BANDWIDTH_OUT: [[u64; 4]; 3] = [
    [0x0B14, 0x0B15, 0x0B16, 0x0B17],
    [0x0B24, 0x0B25, 0x0B26, 0x0B27],
    [0x0B34, 0x0B35, 0x0B36, 0x0B37],
];

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
        let ctrl_addr = IIO_UNIT_BOX_CTL[self.index];
        msr::write(self.core, ctrl_addr, 0x100)?; // Freeze
        msr::write(self.core, ctrl_addr, 0x102)?; // Reset
        Ok(())
    }

    fn unfreeze(&self) -> Result<()> {
        let ctrl_addr = IIO_UNIT_BOX_CTL[self.index];
        msr::write(self.core, ctrl_addr, 0)?;
        Ok(())
    }

    fn program(&self, config: &IioEventConfig) -> Result<()> {
        self.freeze_and_reset()?;

        let ctrl_addrs = [
            IIO_UNIT_CTL0[self.index],
            IIO_UNIT_CTL1[self.index],
            IIO_UNIT_CTL2[self.index],
            IIO_UNIT_CTL3[self.index],
        ];

        for (i, &(event, umask, ch_mask, fc_mask)) in config.events.iter().enumerate() {
            let ctrl_value = (event as u64)
                | ((umask as u64) << 8)
                | ((ch_mask as u64) << 36)
                | ((fc_mask as u64) << 48)
                | (1 << 22); // Enable
            msr::write(self.core, ctrl_addrs[i], ctrl_value)?;
        }

        self.unfreeze()?;
        Ok(())
    }

    fn read_counters(&self) -> Result<[u64; 5]> {
        let ctr_addrs = [
            IIO_UNIT_CTR0[self.index],
            IIO_UNIT_CTR1[self.index],
            IIO_UNIT_CTR2[self.index],
            IIO_UNIT_CTR3[self.index],
            IIO_UNIT_CLK[self.index],
        ];

        let mut values = [0u64; 5];
        let mask = (1u64 << UNCORE_COUNTER_WIDTH) - 1;
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
    pcie_last_values: Option<[[u64; IIO_PCIE_PORT_NUM * 2]; IIO_CHANNEL]>,
    pcie_last_time: Option<Instant>,
}

impl IioMonitor {
    pub fn new(socket: i32) -> Result<Self> {
        let core = (socket as u32) * 16;

        let mut units = Vec::new();
        for i in 0..IIO_CHANNEL {
            units.push(IioCounterUnit::new(core, i)?);
        }

        Ok(Self {
            socket,
            core,
            units,
            event_results: HashMap::new(),
            pcie_last_values: None,
            pcie_last_time: None,
        })
    }

    pub fn collect_metrics(&mut self) -> Result<HashMap<IioMetric, f64>> {
        let mut metrics = HashMap::new();

        // Collect programmable counter metrics
        for event_config in IIO_EVENTS {
            for unit in &self.units {
                unit.program(event_config)?;
            }

            std::thread::sleep(Duration::from_secs(1));

            let mut all_values = Vec::new();
            for unit in &self.units {
                all_values.push(unit.read_counters()?);
            }

            self.event_results
                .insert(event_config.name.to_string(), all_values);
        }

        // Calculate metrics from programmable counters
        self.calculate_programmable_metrics(&mut metrics)?;

        // Collect PCIe free-running counter metrics
        self.collect_pcie_bandwidth(&mut metrics)?;

        Ok(metrics)
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
        let mut current_values = [[0u64; IIO_PCIE_PORT_NUM * 2]; IIO_CHANNEL];

        // Read all PCIe counters
        for ch in 0..IIO_CHANNEL {
            for port in 0..IIO_PCIE_PORT_NUM {
                let in_addr = IIO_PCIE_BANDWIDTH_IN[ch][port];
                let out_addr = IIO_PCIE_BANDWIDTH_OUT[ch][port];

                let in_val = msr::read(self.core, in_addr)? & ((1u64 << IIO_COUNTER_WIDTH) - 1);
                let out_val = msr::read(self.core, out_addr)? & ((1u64 << IIO_COUNTER_WIDTH) - 1);

                current_values[ch][port] = in_val;
                current_values[ch][port + IIO_PCIE_PORT_NUM] = out_val;
            }
        }

        let current_time = Instant::now();

        // Calculate bandwidth if we have previous values
        if let (Some(last_values), Some(last_time)) = (&self.pcie_last_values, self.pcie_last_time)
        {
            let elapsed = current_time.duration_since(last_time).as_secs_f64();

            for ch in 0..IIO_CHANNEL {
                for port in 0..IIO_PCIE_PORT_NUM {
                    // IN bandwidth
                    let in_delta = if current_values[ch][port] >= last_values[ch][port] {
                        current_values[ch][port] - last_values[ch][port]
                    } else {
                        (1u64 << IIO_COUNTER_WIDTH) - last_values[ch][port]
                            + current_values[ch][port]
                    };
                    let in_bandwidth = (in_delta as f64 * CACHELINE_SIZE as f64) / elapsed / 1e9;
                    metrics.insert(IioMetric::PCIeInBandwidth(ch, port), in_bandwidth);

                    // OUT bandwidth
                    let out_idx = port + IIO_PCIE_PORT_NUM;
                    let out_delta = if current_values[ch][out_idx] >= last_values[ch][out_idx] {
                        current_values[ch][out_idx] - last_values[ch][out_idx]
                    } else {
                        (1u64 << IIO_COUNTER_WIDTH) - last_values[ch][out_idx]
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
