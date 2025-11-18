use std::collections::HashMap;

use crate::common::msr;
use crate::config::ExportConfig;
use crate::error::Result;

const MSR_RAPL_POWER_UNIT: u64 = 0x606;
const MSR_PKG_ENERGY_STATUS: u64 = 0x611;
const MSR_PP0_ENERGY_STATUS: u64 = 0x639;
const MSR_DRAM_ENERGY_STATUS: u64 = 0x619;

#[derive(Debug, Clone, Copy, Default)]
pub struct RaplData {
    pub package_energy: f64,
    pub core_energy: f64,
    pub dram_energy: f64,
}

pub struct RaplMonitor {
    config: ExportConfig,
    energy_units: HashMap<i32, f64>,
    socket_to_cpu: HashMap<i32, u32>,
    last_readings: HashMap<i32, RaplData>,
}

impl RaplMonitor {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let mut energy_units = HashMap::new();
        let mut socket_to_cpu = HashMap::new();
        let mut last_readings = HashMap::new();

        for &socket_id in &config.sockets {
            let first_cpu = Self::find_first_cpu_for_socket(&config, socket_id)?;

            let rapl_unit = msr::read_msr(first_cpu, MSR_RAPL_POWER_UNIT)?;
            let energy_unit = 1.0 / (1u64 << ((rapl_unit >> 8) & 0x1F)) as f64;

            energy_units.insert(socket_id, energy_unit);
            socket_to_cpu.insert(socket_id, first_cpu);

            last_readings.insert(socket_id, RaplData::default());
        }

        let mut monitor = Self {
            config,
            energy_units,
            socket_to_cpu,
            last_readings,
        };

        for &socket_id in &monitor.config.sockets {
            let initial = monitor.get_current_energy(socket_id)?;
            monitor.last_readings.insert(socket_id, initial);
        }

        Ok(monitor)
    }

    fn find_first_cpu_for_socket(config: &ExportConfig, socket_id: i32) -> Result<u32> {
        // Try to find a CPU that belongs to the specified socket by checking topology
        for &cpu in &config.cores {
            let topology_path =
                format!("/sys/devices/system/cpu/cpu{cpu}/topology/physical_package_id");

            if let Ok(package_id_str) = std::fs::read_to_string(&topology_path) {
                if let Ok(package_id) = package_id_str.trim().parse::<i32>() {
                    if package_id == socket_id {
                        tracing::debug!(
                            "Found CPU {cpu} for socket {socket_id} (package_id={package_id})"
                        );
                        return Ok(cpu as u32);
                    }
                }
            }
        }

        // Fallback: if we can't determine topology, warn and use first available CPU
        tracing::warn!(
            "Could not find CPU for socket {socket_id} using topology info, using fallback"
        );

        if !config.cores.is_empty() {
            return Ok(config.cores[0] as u32);
        }

        Ok(0)
    }

    fn read_msr(&self, socket: i32, reg: u64) -> Result<u64> {
        let cpu = self.socket_to_cpu[&socket];
        msr::read_msr(cpu, reg)
    }

    fn read_energy_status(&self, socket: i32, msr_addr: u64) -> Result<f64> {
        let raw = self.read_msr(socket, msr_addr)?;
        let energy_unit = self.energy_units[&socket];
        Ok(raw as f64 * energy_unit)
    }

    pub fn get_current_energy(&self, socket: i32) -> Result<RaplData> {
        let package_energy = self.read_energy_status(socket, MSR_PKG_ENERGY_STATUS)?;
        let core_energy = self.read_energy_status(socket, MSR_PP0_ENERGY_STATUS)?;
        let dram_energy = self.read_energy_status(socket, MSR_DRAM_ENERGY_STATUS)?;

        Ok(RaplData {
            package_energy,
            core_energy,
            dram_energy,
        })
    }

    pub fn get_power_consumption(&mut self, socket: i32) -> Result<RaplData> {
        let current = self.get_current_energy(socket)?;
        let last = self.last_readings[&socket];

        let power = RaplData {
            package_energy: current.package_energy - last.package_energy,
            core_energy: current.core_energy - last.core_energy,
            dram_energy: current.dram_energy - last.dram_energy,
        };

        self.last_readings.insert(socket, current);

        Ok(power)
    }
}
