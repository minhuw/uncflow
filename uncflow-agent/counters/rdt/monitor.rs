use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use crate::common::{cpuid, msr};
use crate::config::ExportConfig;
use crate::error::{Result, UncflowError};

const IA32_PQR_ASSOC: u64 = 0xC8F;
const IA32_QM_EVTSEL: u64 = 0xC8D;
const IA32_QM_CTR: u64 = 0xC8E;

const LLC_OCCUPANCY_EVENT: u64 = 0x01;
const LOCAL_MEM_BW_EVENT: u64 = 0x02;
const REMOTE_MEM_BW_EVENT: u64 = 0x03;

const RMID_MAX: usize = 256;

#[derive(Debug, Clone)]
struct SocketInfo {
    socket_id: i32,
    cores: Vec<i32>,
    last_local_bw: u64,
    last_remote_bw: u64,
}

pub struct RdtMonitor {
    config: ExportConfig,
    mbm_scaling_factor: u32,
    local_memory_bandwidth: Vec<u64>,
    remote_memory_bandwidth: Vec<u64>,
    llc_occupancy: Vec<u64>,
    prev_local_counters: Vec<u64>,
    prev_remote_counters: Vec<u64>,
    core_to_rmid: Vec<u32>,
    rmid_used: Vec<bool>,
    sockets: Vec<SocketInfo>,
}

impl RdtMonitor {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let mbm_scaling_factor = cpuid::get_mbm_scaling_factor()?;

        let max_core = config.cores.iter().max().copied().unwrap_or(0);
        if max_core < 0 {
            return Err(UncflowError::RdtError(
                "Negative core IDs are not supported".to_string(),
            ));
        }

        let vector_size = (max_core + 1) as usize;
        let local_memory_bandwidth = vec![0; vector_size];
        let remote_memory_bandwidth = vec![0; vector_size];
        let llc_occupancy = vec![0; vector_size];
        let prev_local_counters = vec![0; vector_size];
        let prev_remote_counters = vec![0; vector_size];
        let core_to_rmid = vec![0; vector_size];
        let rmid_used = vec![false; RMID_MAX];

        let mut monitor = Self {
            config,
            mbm_scaling_factor,
            local_memory_bandwidth,
            remote_memory_bandwidth,
            llc_occupancy,
            prev_local_counters,
            prev_remote_counters,
            core_to_rmid,
            rmid_used,
            sockets: Vec::new(),
        };

        monitor.initialize_socket_info()?;
        Ok(monitor)
    }

    fn initialize_socket_info(&mut self) -> Result<()> {
        let mut socket_cores: HashMap<i32, Vec<i32>> = HashMap::new();

        for &core in &self.config.cores {
            let socket_id = Self::read_socket_id(core)?;
            socket_cores.entry(socket_id).or_default().push(core);
        }

        for (socket_id, cores) in socket_cores {
            self.sockets.push(SocketInfo {
                socket_id,
                cores,
                last_local_bw: 0,
                last_remote_bw: 0,
            });
        }

        Ok(())
    }

    fn read_socket_id(core: i32) -> Result<i32> {
        let path = format!("/sys/devices/system/cpu/cpu{core}/topology/physical_package_id");
        let mut file = File::open(&path).map_err(|e| {
            UncflowError::RdtError(format!("Cannot read CPU topology for core {core}: {e}"))
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|e| {
            UncflowError::RdtError(format!("Cannot read socket ID for core {core}: {e}"))
        })?;

        contents
            .trim()
            .parse()
            .map_err(|e| UncflowError::RdtError(format!("Invalid socket ID for core {core}: {e}")))
    }

    fn allocate_rmid(&mut self) -> Result<u32> {
        for i in 1..RMID_MAX {
            if !self.rmid_used[i] {
                self.rmid_used[i] = true;
                return Ok(i as u32);
            }
        }
        Err(UncflowError::RdtError(
            "No free RMIDs available".to_string(),
        ))
    }

    fn free_rmid(&mut self, rmid: u32) {
        if rmid > 0 && (rmid as usize) < RMID_MAX {
            self.rmid_used[rmid as usize] = false;
        }
    }

    fn assign_rmid_to_core(&mut self, core_id: i32, rmid: u32) -> Result<()> {
        let current_assoc = msr::read_msr(core_id as u32, IA32_PQR_ASSOC)?;
        let new_assoc = (current_assoc & !0x3FF) | (rmid as u64);
        msr::write_msr(core_id as u32, IA32_PQR_ASSOC, new_assoc)?;
        self.core_to_rmid[core_id as usize] = rmid;
        Ok(())
    }

    pub fn initialize(&mut self) -> Result<()> {
        let cores = self.config.cores.clone();
        for core in cores {
            let rmid = self.allocate_rmid()?;
            self.assign_rmid_to_core(core, rmid)?;

            let label = self
                .config
                .core_labels
                .get(&core)
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            tracing::info!(
                "Initialized MBM monitoring for core {} ({}) with RMID {}",
                core,
                label,
                rmid
            );
        }
        Ok(())
    }

    fn update_socket_metrics(&mut self, socket_idx: usize) -> Result<()> {
        let socket = &self.sockets[socket_idx];
        let monitoring_core = socket.cores[0] as u32;

        let mut socket_local_bw = 0u64;
        let mut socket_remote_bw = 0u64;

        for &core in &socket.cores {
            let rmid = self.core_to_rmid[core as usize];

            msr::write_msr(
                monitoring_core,
                IA32_QM_EVTSEL,
                ((rmid as u64) << 32) | LLC_OCCUPANCY_EVENT,
            )?;
            let llc_counter = msr::read_msr(monitoring_core, IA32_QM_CTR)?;
            self.llc_occupancy[core as usize] = llc_counter * (self.mbm_scaling_factor as u64);

            msr::write_msr(
                monitoring_core,
                IA32_QM_EVTSEL,
                ((rmid as u64) << 32) | LOCAL_MEM_BW_EVENT,
            )?;
            let local_counter = msr::read_msr(monitoring_core, IA32_QM_CTR)?;

            msr::write_msr(
                monitoring_core,
                IA32_QM_EVTSEL,
                ((rmid as u64) << 32) | REMOTE_MEM_BW_EVENT,
            )?;
            let remote_counter = msr::read_msr(monitoring_core, IA32_QM_CTR)?;

            let local_delta = if local_counter >= self.prev_local_counters[core as usize] {
                local_counter - self.prev_local_counters[core as usize]
            } else {
                local_counter
            };

            let remote_delta = if remote_counter >= self.prev_remote_counters[core as usize] {
                remote_counter - self.prev_remote_counters[core as usize]
            } else {
                remote_counter
            };

            self.local_memory_bandwidth[core as usize] =
                local_delta * (self.mbm_scaling_factor as u64);
            self.remote_memory_bandwidth[core as usize] =
                remote_delta * (self.mbm_scaling_factor as u64);

            self.prev_local_counters[core as usize] = local_counter;
            self.prev_remote_counters[core as usize] = remote_counter;

            socket_local_bw += self.local_memory_bandwidth[core as usize];
            socket_remote_bw += self.remote_memory_bandwidth[core as usize];
        }

        self.sockets[socket_idx].last_local_bw = socket_local_bw;
        self.sockets[socket_idx].last_remote_bw = socket_remote_bw;

        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        for i in 0..self.sockets.len() {
            if let Err(e) = self.update_socket_metrics(i) {
                tracing::error!(
                    "Failed to update socket {} metrics: {}",
                    self.sockets[i].socket_id,
                    e
                );
            }
        }
        Ok(())
    }

    pub fn refresh_rmids(&mut self) -> Result<()> {
        let cores = self.config.cores.clone();
        for core in cores {
            let rmid = self.core_to_rmid[core as usize];
            if rmid != 0 {
                self.assign_rmid_to_core(core, rmid)?;
            }
        }
        Ok(())
    }

    pub fn get_metrics(&self, core_id: i32) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();

        if self.config.cores.contains(&core_id) {
            let idx = core_id as usize;
            metrics.insert(
                "LocalMemoryBandwidth".to_string(),
                self.local_memory_bandwidth[idx] as f64,
            );
            metrics.insert(
                "RemoteMemoryBandwidth".to_string(),
                self.remote_memory_bandwidth[idx] as f64,
            );
            metrics.insert(
                "TotalMemoryBandwidth".to_string(),
                (self.local_memory_bandwidth[idx] + self.remote_memory_bandwidth[idx]) as f64,
            );
            metrics.insert(
                "CMTLLCOccupancy".to_string(),
                self.llc_occupancy[idx] as f64,
            );
        }

        metrics
    }

    /// Get aggregated socket-level metrics
    pub fn get_socket_metrics(&self, socket_id: i32) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();

        if let Some(socket_info) = self.sockets.iter().find(|s| s.socket_id == socket_id) {
            metrics.insert(
                "LocalMemoryBandwidth".to_string(),
                socket_info.last_local_bw as f64,
            );
            metrics.insert(
                "RemoteMemoryBandwidth".to_string(),
                socket_info.last_remote_bw as f64,
            );
            metrics.insert(
                "TotalMemoryBandwidth".to_string(),
                (socket_info.last_local_bw + socket_info.last_remote_bw) as f64,
            );

            // Aggregate LLC occupancy for all cores in this socket
            let mut total_llc_occupancy = 0u64;
            for &core in &socket_info.cores {
                total_llc_occupancy += self.llc_occupancy[core as usize];
            }
            metrics.insert("CMTLLCOccupancy".to_string(), total_llc_occupancy as f64);
        }

        metrics
    }
}

impl Drop for RdtMonitor {
    fn drop(&mut self) {
        let cores = self.config.cores.clone();
        for core in cores {
            let rmid = self.core_to_rmid[core as usize];
            if rmid != 0 {
                self.free_rmid(rmid);
            }
        }
    }
}
