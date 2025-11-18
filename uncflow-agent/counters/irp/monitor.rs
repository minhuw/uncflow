// IRP (IO Request Processing) Monitor

use crate::common::{arch::CPU_ARCH, msr, pci};
use crate::error::{Result, UncflowError};
use crate::metrics::irp::IrpMetric;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Skylake IRP MSR addresses (3 IRP units per socket)
const IRP_UNIT_CTRL: [u64; 3] = [0x0A78, 0x0A98, 0x0AB8];
const _IRP_UNIT_STATUS: [u64; 3] = [0x0A7F, 0x0A9F, 0x0ABF];
const IRP_CTR0: [u64; 3] = [0x0A79, 0x0A99, 0x0AB9];
const IRP_CTR1: [u64; 3] = [0x0A7A, 0x0A9A, 0x0ABA];
const IRP_CTRL0: [u64; 3] = [0x0A7B, 0x0A9B, 0x0ABB];
const IRP_CTRL1: [u64; 3] = [0x0A7C, 0x0A9C, 0x0ABC];

// Haswell/Broadwell IRP PCI addresses
const IRP_DEVICE: u32 = 5;
const IRP_FUNCTION: u32 = 6;
const IRP_DEVICE_ID: u32 = 0x6F39;
const IRP_UNIT_STATUS_ADDR: u32 = 0xF8;
const IRP_UNIT_CTL_ADDR: u32 = 0xF4;
const IRP_CTR_ADDR: [u32; 4] = [0xA0, 0xB0, 0xB8, 0xC0];
const IRP_CTL_ADDR: [u32; 4] = [0xD8, 0xDC, 0xE0, 0xE4];

const UNCORE_COUNTER_WIDTH: u64 = 48;
const IRP_PCI_COUNTER_WIDTH: u32 = 32;
const CACHELINE_SIZE: u64 = 64;

// IRP Event configurations
#[derive(Debug, Clone)]
struct IrpEventConfig {
    name: &'static str,
    event0: u8,
    umask0: u8,
    event1: u8,
    umask1: u8,
}

const IRP_EVENTS: &[IrpEventConfig] = &[
    IrpEventConfig {
        name: "All",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0xFF,
    },
    IrpEventConfig {
        name: "Clockticks",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x01,
        umask1: 0x00,
    },
    IrpEventConfig {
        name: "PCIeRead",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0x01,
    },
    IrpEventConfig {
        name: "RFO",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0x08,
    },
    IrpEventConfig {
        name: "PCIItoM",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0x10,
    },
    IrpEventConfig {
        name: "WbMtoI",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0x40,
    },
    IrpEventConfig {
        name: "CLFlush",
        event0: 0x0F,
        umask0: 0x01,
        event1: 0x10,
        umask1: 0x80,
    },
];

// MSR-based IRP counter unit (Skylake)
#[derive(Debug)]
struct IrpMsrCounterUnit {
    core: u32,
    index: usize,
}

impl IrpMsrCounterUnit {
    fn new(core: u32, index: usize) -> Result<Self> {
        Ok(Self { core, index })
    }

    fn freeze_and_reset(&self) -> Result<()> {
        let ctrl_addr = IRP_UNIT_CTRL[self.index];
        msr::write(self.core, ctrl_addr, 0x100)?; // Freeze
        msr::write(self.core, ctrl_addr, 0x102)?; // Reset
        Ok(())
    }

    fn unfreeze(&self) -> Result<()> {
        let ctrl_addr = IRP_UNIT_CTRL[self.index];
        msr::write(self.core, ctrl_addr, 0)?;
        Ok(())
    }

    fn program(&self, config: &IrpEventConfig) -> Result<()> {
        self.freeze_and_reset()?;

        let ctrl0_value = ((config.umask0 as u64) << 8) | (config.event0 as u64) | (1 << 22);
        msr::write(self.core, IRP_CTRL0[self.index], ctrl0_value)?;

        let ctrl1_value = ((config.umask1 as u64) << 8) | (config.event1 as u64) | (1 << 22);
        msr::write(self.core, IRP_CTRL1[self.index], ctrl1_value)?;

        self.unfreeze()?;
        Ok(())
    }

    fn read_counters(&self) -> Result<[u64; 2]> {
        let ctr0 = msr::read(self.core, IRP_CTR0[self.index])?;
        let ctr1 = msr::read(self.core, IRP_CTR1[self.index])?;
        let mask = (1u64 << UNCORE_COUNTER_WIDTH) - 1;
        Ok([ctr0 & mask, ctr1 & mask])
    }
}

// PCI-based IRP counter unit (Haswell/Broadwell)
#[derive(Debug)]
struct IrpPciCounterUnit {
    pci_addr: pci::PciConfigAddress,
}

impl IrpPciCounterUnit {
    fn new(socket: u32) -> Result<Self> {
        let pci_addr = pci::PciConfigAddress {
            socket,
            device: IRP_DEVICE,
            function: IRP_FUNCTION,
            device_id: IRP_DEVICE_ID,
        };

        // Verify the device exists
        let pci_inst = pci::Pci::instance();
        let vendor_device = pci_inst.read32(&pci_addr, 0)?;
        let vendor = vendor_device & 0xFFFF;
        let device = (vendor_device >> 16) & 0xFFFF;

        if vendor != 0x8086 {
            return Err(UncflowError::PciError(format!(
                "IRP device not found for socket {socket}: invalid vendor {vendor:04X}"
            )));
        }

        if device != IRP_DEVICE_ID {
            return Err(UncflowError::PciError(format!(
                "IRP device ID mismatch for socket {socket}: expected {IRP_DEVICE_ID:04X}, got {device:04X}"
            )));
        }

        Ok(Self { pci_addr })
    }

    fn freeze_and_reset(&self) -> Result<()> {
        let pci = pci::Pci::instance();
        pci.write32(&self.pci_addr, IRP_UNIT_CTL_ADDR, 0x100)?; // Freeze
        pci.write32(&self.pci_addr, IRP_UNIT_CTL_ADDR, 0x102)?; // Reset
        Ok(())
    }

    fn unfreeze(&self) -> Result<()> {
        let pci = pci::Pci::instance();
        pci.write32(&self.pci_addr, IRP_UNIT_CTL_ADDR, 0)?;
        Ok(())
    }

    fn program(&self, config0: &IrpEventConfig, config1: &IrpEventConfig) -> Result<()> {
        self.freeze_and_reset()?;

        let pci = pci::Pci::instance();

        // Program counter 0 with config0.event0
        let ctrl00_value = ((config0.umask0 as u32) << 8) | (config0.event0 as u32) | (1 << 22);
        pci.write32(&self.pci_addr, IRP_CTL_ADDR[0], ctrl00_value)?;

        // Program counter 1 with config0.event1
        let ctrl01_value = ((config0.umask1 as u32) << 8) | (config0.event1 as u32) | (1 << 22);
        pci.write32(&self.pci_addr, IRP_CTL_ADDR[1], ctrl01_value)?;

        // Program counter 2 with config1.event0
        let ctrl10_value = ((config1.umask0 as u32) << 8) | (config1.event0 as u32) | (1 << 22);
        pci.write32(&self.pci_addr, IRP_CTL_ADDR[2], ctrl10_value)?;

        // Program counter 3 with config1.event1
        let ctrl11_value = ((config1.umask1 as u32) << 8) | (config1.event1 as u32) | (1 << 22);
        pci.write32(&self.pci_addr, IRP_CTL_ADDR[3], ctrl11_value)?;

        self.unfreeze()?;
        Ok(())
    }

    fn read_counters(&self) -> Result<[u64; 4]> {
        let pci = pci::Pci::instance();

        // Check and clear overflow
        let status = pci.read32(&self.pci_addr, IRP_UNIT_STATUS_ADDR)?;
        if status & 0xF != 0 {
            pci.write32(&self.pci_addr, IRP_UNIT_STATUS_ADDR, status & 0xF)?;
        }

        let mask = (1u64 << IRP_PCI_COUNTER_WIDTH) - 1;
        let ctr0 = (pci.read32(&self.pci_addr, IRP_CTR_ADDR[0])? as u64) & mask;
        let ctr1 = (pci.read32(&self.pci_addr, IRP_CTR_ADDR[1])? as u64) & mask;
        let ctr2 = (pci.read32(&self.pci_addr, IRP_CTR_ADDR[2])? as u64) & mask;
        let ctr3 = (pci.read32(&self.pci_addr, IRP_CTR_ADDR[3])? as u64) & mask;

        Ok([ctr0, ctr1, ctr2, ctr3])
    }
}

// Unified counter interface
#[derive(Debug)]
enum IrpCounterUnit {
    Msr(IrpMsrCounterUnit),
    Pci(IrpPciCounterUnit),
}

impl IrpCounterUnit {
    fn program(&self, config: &IrpEventConfig) -> Result<()> {
        match self {
            IrpCounterUnit::Msr(unit) => unit.program(config),
            IrpCounterUnit::Pci(_) => {
                // PCI units need two configs, so we'll handle this differently
                Ok(())
            }
        }
    }

    fn program_pci_pair(&self, config0: &IrpEventConfig, config1: &IrpEventConfig) -> Result<()> {
        match self {
            IrpCounterUnit::Pci(unit) => unit.program(config0, config1),
            IrpCounterUnit::Msr(_) => Ok(()),
        }
    }

    fn read_counters(&self) -> Result<Vec<u64>> {
        match self {
            IrpCounterUnit::Msr(unit) => {
                let values = unit.read_counters()?;
                Ok(vec![values[0], values[1]])
            }
            IrpCounterUnit::Pci(unit) => {
                let values = unit.read_counters()?;
                Ok(vec![values[0], values[1], values[2], values[3]])
            }
        }
    }
}

#[derive(Debug)]
pub struct IrpMonitor {
    socket: i32,
    units: Vec<IrpCounterUnit>,
    event_results: HashMap<String, [u64; 2]>,
    measure_start: Option<Instant>,
    measure_duration: Duration,
}

impl IrpMonitor {
    pub fn new(socket: i32) -> Result<Self> {
        let arch = *CPU_ARCH;
        let mut units = Vec::new();

        match arch {
            crate::common::arch::CpuArchitecture::Skylake
            | crate::common::arch::CpuArchitecture::CascadeLake
            | crate::common::arch::CpuArchitecture::IceLake => {
                // MSR-based counters for Skylake and newer
                let core = (socket as u32) * 16;
                for i in 0..3 {
                    units.push(IrpCounterUnit::Msr(IrpMsrCounterUnit::new(core, i)?));
                }
            }
            crate::common::arch::CpuArchitecture::Haswell
            | crate::common::arch::CpuArchitecture::Broadwell => {
                // PCI-based counters for Haswell/Broadwell
                units.push(IrpCounterUnit::Pci(IrpPciCounterUnit::new(socket as u32)?));
            }
            _ => {
                return Err(UncflowError::UnsupportedArchitecture(format!(
                    "IRP monitoring not supported on {arch:?}"
                )));
            }
        }

        Ok(Self {
            socket,
            units,
            event_results: HashMap::new(),
            measure_start: None,
            measure_duration: Duration::from_secs(1),
        })
    }

    pub fn collect_metrics(&mut self) -> Result<HashMap<IrpMetric, f64>> {
        let mut metrics = HashMap::new();

        match self.units.first() {
            Some(IrpCounterUnit::Msr(_)) => {
                // MSR mode: iterate through all event configurations
                for event_config in IRP_EVENTS {
                    for unit in &self.units {
                        unit.program(event_config)?;
                    }

                    self.measure_start = Some(Instant::now());
                    std::thread::sleep(self.measure_duration);

                    let mut aggregated = [0u64, 0u64];
                    for unit in &self.units {
                        let values = unit.read_counters()?;
                        aggregated[0] += values[0];
                        aggregated[1] += values[1];
                    }

                    let elapsed = self.measure_start.unwrap().elapsed();
                    self.event_results
                        .insert(event_config.name.to_string(), aggregated);

                    self.calculate_event_metrics(
                        event_config.name,
                        &aggregated,
                        elapsed,
                        &mut metrics,
                    );
                }
            }
            Some(IrpCounterUnit::Pci(_)) => {
                // PCI mode: program in pairs (4 counters at once)
                for i in (0..IRP_EVENTS.len()).step_by(2) {
                    if i + 1 < IRP_EVENTS.len() {
                        let config0 = &IRP_EVENTS[i];
                        let config1 = &IRP_EVENTS[i + 1];

                        for unit in &self.units {
                            unit.program_pci_pair(config0, config1)?;
                        }

                        self.measure_start = Some(Instant::now());
                        std::thread::sleep(self.measure_duration);

                        for unit in &self.units {
                            let values = unit.read_counters()?;
                            let elapsed = self.measure_start.unwrap().elapsed();

                            // First pair of counters (config0)
                            let aggregated0 = [values[0], values[1]];
                            self.event_results
                                .insert(config0.name.to_string(), aggregated0);
                            self.calculate_event_metrics(
                                config0.name,
                                &aggregated0,
                                elapsed,
                                &mut metrics,
                            );

                            // Second pair of counters (config1)
                            let aggregated1 = [values[2], values[3]];
                            self.event_results
                                .insert(config1.name.to_string(), aggregated1);
                            self.calculate_event_metrics(
                                config1.name,
                                &aggregated1,
                                elapsed,
                                &mut metrics,
                            );
                        }
                    }
                }
            }
            None => {
                return Err(UncflowError::InvalidConfiguration(
                    "No IRP units available".to_string(),
                ));
            }
        }

        Ok(metrics)
    }

    fn calculate_event_metrics(
        &self,
        event_name: &str,
        values: &[u64; 2],
        duration: Duration,
        metrics: &mut HashMap<IrpMetric, f64>,
    ) {
        let elapsed_ns = duration.as_nanos() as f64;
        let elapsed_s = duration.as_secs_f64();

        match event_name {
            "Clockticks" => {
                // Calculate frequency
                let frequency = values[1] as f64 / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPFrequency, frequency);

                // Calculate occupancy
                if let Some(occupancy_values) = self.event_results.get("All") {
                    let occupancy = occupancy_values[0] as f64 / ((frequency * 1e9) * elapsed_s);
                    metrics.insert(IrpMetric::IRPAnyOccupancy, occupancy);
                }
            }
            "All" => {
                // Calculate latency (requires Clockticks to be collected)
                if let Some(clockticks) = self.event_results.get("Clockticks") {
                    let latency = if values[1] > 0 {
                        (values[0] as f64) / (values[1] as f64)
                            * (clockticks[1] as f64 / elapsed_ns)
                    } else {
                        0.0
                    };
                    metrics.insert(IrpMetric::IRPLatency, latency);
                }

                // Bandwidth calculation
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPAllBandwidth, bandwidth);
            }
            "PCIeRead" => {
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPPCIeReadBandwidth, bandwidth);
            }
            "RFO" => {
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPRFOBandwidth, bandwidth);
            }
            "PCIItoM" => {
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPPCIItoMBandwidth, bandwidth);
            }
            "WbMtoI" => {
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPWbMtoIBandwidth, bandwidth);
            }
            "CLFlush" => {
                let bandwidth = (values[1] as f64 * CACHELINE_SIZE as f64) / elapsed_s / 1e9;
                metrics.insert(IrpMetric::IRPCLFlushBandwidth, bandwidth);
            }
            _ => {}
        }
    }

    pub fn socket(&self) -> i32 {
        self.socket
    }
}
