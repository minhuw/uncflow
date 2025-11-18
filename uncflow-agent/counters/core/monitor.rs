use std::collections::HashMap;

use crate::common::msr;
use crate::config::ExportConfig;
use crate::counters::core::events::*;
use crate::error::Result;

#[derive(Debug, Clone, Default)]
pub struct CoreMetrics {
    pub instructions: u64,
    pub cycles: u64,
    pub ref_cycles: u64,
    pub llc_ref: u64,
    pub llc_miss: u64,
    pub l2_ref: u64,
    pub l2_miss: u64,
    pub l2_prefetch_miss: u64,
    pub l2_prefetch_hit: u64,
    pub l2_out_silent: u64,
    pub l2_out_non_silent: u64,
    pub l2_in: u64,
    pub l2_writeback: u64,
    pub tsc_start: u64,
    pub tsc_end: u64,
}

pub struct CoreMonitor {
    config: ExportConfig,
    cpu_frequency: f64,
    prev_metrics: HashMap<i32, CoreMetrics>,
    programmable_events: Vec<PmuEvent>,
}

impl CoreMonitor {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let cpu_frequency = Self::get_cpu_frequency()?;
        tracing::info!("Detected CPU frequency: {:.2} GHz", cpu_frequency / 1e9);

        // Get the default event set (architecture-aware)
        let programmable_events = crate::counters::core::events::get_default_event_set();

        tracing::info!(
            "Selected {} PMU events for architecture: {}",
            programmable_events.len(),
            crate::common::CPU_ARCH.name()
        );

        let prev_metrics = HashMap::new();

        Ok(Self {
            config,
            cpu_frequency,
            prev_metrics,
            programmable_events,
        })
    }

    fn get_cpu_frequency() -> Result<f64> {
        // Read MSR_PLATFORM_INFO to get base frequency
        let platform_info = msr::read_msr(0, MSR_PLATFORM_INFO)?;
        let max_non_turbo_ratio = (platform_info >> 8) & 0xFF;
        let frequency = (max_non_turbo_ratio as f64) * 100_000_000.0; // 100 MHz per ratio
        Ok(frequency)
    }

    pub fn initialize(&mut self) -> Result<()> {
        let cores = self.config.cores.clone();
        for core in cores {
            self.initialize_core(core)?;
            tracing::info!("Initialized PMU for core {}", core);
        }
        Ok(())
    }

    fn initialize_core(&self, core: i32) -> Result<()> {
        let core_u32 = core as u32;

        // Disable all counters
        msr::write_msr(core_u32, IA32_PERF_GLOBAL_CTRL, 0)?;

        // Configure fixed counters (instructions, cycles, ref cycles)
        // Enable user mode counting for all 3 fixed counters
        let fixed_ctrl = 0x333u64; // User mode for CTR0, CTR1, CTR2
        msr::write_msr(core_u32, IA32_FIXED_CTR_CTRL, fixed_ctrl)?;

        // Program the programmable counters
        for (i, event) in self.programmable_events.iter().enumerate() {
            let perfevtsel_addr = IA32_PERFEVTSEL0 + (i as u64);
            let event_config = event.encode_for_perfevtsel(true, false);
            msr::write_msr(core_u32, perfevtsel_addr, event_config)?;
        }

        // Clear all counters
        msr::write_msr(core_u32, IA32_FIXED_CTR0, 0)?;
        msr::write_msr(core_u32, IA32_FIXED_CTR1, 0)?;
        msr::write_msr(core_u32, IA32_FIXED_CTR2, 0)?;
        for i in 0..4 {
            let pmc_addr = IA32_PMC0 + (i as u64);
            msr::write_msr(core_u32, pmc_addr, 0)?;
        }

        // Enable all counters: 3 fixed + 4 programmable
        let global_ctrl = (0x7u64 << 32) | 0xFu64; // Fixed[2:0] + PMC[3:0]
        msr::write_msr(core_u32, IA32_PERF_GLOBAL_CTRL, global_ctrl)?;

        Ok(())
    }

    fn read_core_counters(&self, core: i32) -> Result<CoreMetrics> {
        let core_u32 = core as u32;

        // Read TSC first
        let tsc_start = msr::read_msr(core_u32, IA32_TIME_STAMP_COUNTER)?;

        // Read fixed counters
        let instructions = msr::read_msr(core_u32, IA32_FIXED_CTR0)?;
        let cycles = msr::read_msr(core_u32, IA32_FIXED_CTR1)?;
        let ref_cycles = msr::read_msr(core_u32, IA32_FIXED_CTR2)?;

        // Read programmable counters (matching our event programming)
        let llc_ref = msr::read_msr(core_u32, IA32_PMC0)?;
        let llc_miss = msr::read_msr(core_u32, IA32_PMC1)?;
        let l2_miss = msr::read_msr(core_u32, IA32_PMC2)?;
        let l2_ref = msr::read_msr(core_u32, IA32_PMC3)?;

        // For now, set other L2 metrics to 0 (would need event multiplexing)
        let metrics = CoreMetrics {
            instructions,
            cycles,
            ref_cycles,
            llc_ref,
            llc_miss,
            l2_ref,
            l2_miss,
            l2_prefetch_miss: 0,
            l2_prefetch_hit: 0,
            l2_out_silent: 0,
            l2_out_non_silent: 0,
            l2_in: 0,
            l2_writeback: 0,
            tsc_start,
            tsc_end: tsc_start,
        };

        Ok(metrics)
    }

    pub fn collect(&mut self) -> Result<()> {
        let cores = self.config.cores.clone();
        for core in cores {
            let metrics = self.read_core_counters(core)?;
            self.prev_metrics.insert(core, metrics);
        }
        Ok(())
    }

    pub fn get_metrics(&self, core: i32) -> HashMap<String, f64> {
        let mut result = HashMap::new();

        if let Some(metrics) = self.prev_metrics.get(&core) {
            // Basic counters
            result.insert("instructions".to_string(), metrics.instructions as f64);
            result.insert("cycles".to_string(), metrics.cycles as f64);

            // Derived metrics
            let ipc = if metrics.cycles > 0 {
                metrics.instructions as f64 / metrics.cycles as f64
            } else {
                0.0
            };
            result.insert("IPC".to_string(), ipc);

            // L3 (LLC) metrics
            result.insert("L3CacheMissNum".to_string(), metrics.llc_miss as f64);
            result.insert("L3CacheRef".to_string(), metrics.llc_ref as f64);

            let l3_hit_ratio = if metrics.llc_ref > 0 {
                1.0 - (metrics.llc_miss as f64 / metrics.llc_ref as f64)
            } else {
                0.0
            };
            result.insert("L3CacheHitRatio".to_string(), l3_hit_ratio);

            // L3 MPI (Misses Per Instruction)
            let l3_mpi = if metrics.instructions > 0 {
                (metrics.llc_miss as f64) / (metrics.instructions as f64)
            } else {
                0.0
            };
            result.insert("L3MPI".to_string(), l3_mpi);

            // L2 metrics
            result.insert("L2CacheMissNum".to_string(), metrics.l2_miss as f64);
            result.insert("L2CacheRef".to_string(), metrics.l2_ref as f64);

            let l2_hit_ratio = if metrics.l2_ref > 0 {
                1.0 - (metrics.l2_miss as f64 / metrics.l2_ref as f64)
            } else {
                0.0
            };
            result.insert("L2CacheHitRatio".to_string(), l2_hit_ratio);

            // L2 MPI
            let l2_mpi = if metrics.instructions > 0 {
                (metrics.l2_miss as f64) / (metrics.instructions as f64)
            } else {
                0.0
            };
            result.insert("L2MPI".to_string(), l2_mpi);

            // Elapsed time (approximate from ref cycles)
            let elapsed_time = if self.cpu_frequency > 0.0 {
                (metrics.ref_cycles as f64) / self.cpu_frequency
            } else {
                0.0
            };
            result.insert("elapsedTime".to_string(), elapsed_time);

            // Other L2 metrics (currently 0, would need event multiplexing)
            result.insert(
                "L2PrefetchMiss".to_string(),
                metrics.l2_prefetch_miss as f64,
            );
            result.insert("L2PrefetchHit".to_string(), metrics.l2_prefetch_hit as f64);
            result.insert("L2OutSilent".to_string(), metrics.l2_out_silent as f64);
            result.insert(
                "L2OutNonSilent".to_string(),
                metrics.l2_out_non_silent as f64,
            );
            result.insert("L2In".to_string(), metrics.l2_in as f64);
            result.insert("L2Writeback".to_string(), metrics.l2_writeback as f64);
        }

        result
    }
}

impl Drop for CoreMonitor {
    fn drop(&mut self) {
        // Disable all counters on cleanup
        let cores = self.config.cores.clone();
        for core in cores {
            let _ = msr::write_msr(core as u32, IA32_PERF_GLOBAL_CTRL, 0);
        }
    }
}
