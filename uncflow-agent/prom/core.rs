use prometheus::{Gauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::counters::core::CoreMonitor;
use crate::error::Result;
use crate::metrics::core::CoreMetric;

pub struct CoreMetricExporter {
    config: ExportConfig,
    registry: Arc<Registry>,
    monitor: Arc<parking_lot::Mutex<CoreMonitor>>,
    core_gauges: HashMap<CoreMetric, HashMap<i32, Gauge>>,
}

impl CoreMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());

        let mut monitor = CoreMonitor::new(config.clone())?;
        monitor.initialize()?;

        let monitor = Arc::new(parking_lot::Mutex::new(monitor));

        let mut exporter = Self {
            config: config.clone(),
            registry: Arc::clone(&registry),
            monitor,
            core_gauges: HashMap::new(),
        };

        exporter.register_metrics()?;

        Ok(exporter)
    }

    fn register_metrics(&mut self) -> Result<()> {
        for metric in CoreMetric::all() {
            let opts =
                prometheus::Opts::new(metric.name(), format!("Core {} measurement", metric.name()));

            let mut core_map = HashMap::new();
            for &core_id in &self.config.cores {
                let label = self
                    .config
                    .core_labels
                    .get(&core_id)
                    .map(|s| s.as_str())
                    .unwrap_or("unknown");

                let gauge = Gauge::with_opts(
                    opts.clone()
                        .const_label("core", core_id.to_string())
                        .const_label("core_label", label),
                )?;
                self.registry.register(Box::new(gauge.clone()))?;
                core_map.insert(core_id, gauge);
            }
            self.core_gauges.insert(metric, core_map);
        }

        Ok(())
    }

    async fn collect_loop(
        config: ExportConfig,
        monitor: Arc<parking_lot::Mutex<CoreMonitor>>,
        core_gauges: HashMap<CoreMetric, HashMap<i32, Gauge>>,
    ) {
        tracing::warn!("Starting Core PMU export thread");

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            {
                let mut mon = monitor.lock();
                if let Err(e) = mon.collect() {
                    tracing::error!("Failed to collect core metrics: {}", e);
                    continue;
                }
            }

            for &core_id in &config.cores {
                let mon = monitor.lock();
                let metrics = mon.get_metrics(core_id);
                drop(mon);

                // Update gauges based on metric name
                for (metric_name, value) in metrics {
                    let metric_enum = match metric_name.as_str() {
                        "IPC" => Some(CoreMetric::IPC),
                        "instructions" => Some(CoreMetric::Instructions),
                        "cycles" => Some(CoreMetric::Cycles),
                        "L3CacheMissNum" => Some(CoreMetric::L3CacheMiss),
                        "L3CacheRef" => Some(CoreMetric::L3CacheRef),
                        "L2CacheMissNum" => Some(CoreMetric::L2CacheMiss),
                        "L2CacheRef" => Some(CoreMetric::L2CacheRef),
                        "L3CacheHitRatio" => Some(CoreMetric::L3CacheHitRatio),
                        "L2CacheHitRatio" => Some(CoreMetric::L2CacheHitRatio),
                        "L3MPI" => Some(CoreMetric::L3MPI),
                        "L2MPI" => Some(CoreMetric::L2MPI),
                        "elapsedTime" => Some(CoreMetric::ElapsedTime),
                        "L2PrefetchMiss" => Some(CoreMetric::L2PrefetchMiss),
                        "L2PrefetchHit" => Some(CoreMetric::L2PrefetchHit),
                        "L2OutSilent" => Some(CoreMetric::L2OutSilent),
                        "L2OutNonSilent" => Some(CoreMetric::L2OutNonSilent),
                        "L2In" => Some(CoreMetric::L2In),
                        "L2Writeback" => Some(CoreMetric::L2Writeback),
                        _ => None,
                    };

                    if let Some(metric) = metric_enum {
                        if let Some(gauge) = core_gauges.get(&metric).and_then(|m| m.get(&core_id))
                        {
                            gauge.set(value);
                        }
                    }
                }
            }
        }
    }

    pub fn start(&self) -> JoinHandle<()> {
        let config = self.config.clone();
        let monitor = Arc::clone(&self.monitor);
        let core_gauges = self.core_gauges.clone();

        tokio::spawn(Self::collect_loop(config, monitor, core_gauges))
    }

    /// Collect metrics once (called by orchestrator)
    pub async fn collect(&self) {
        {
            let mut mon = self.monitor.lock();
            if let Err(e) = mon.collect() {
                tracing::error!("Failed to collect core metrics: {}", e);
                return;
            }
        }

        for &core_id in &self.config.cores {
            let mon = self.monitor.lock();
            let metrics = mon.get_metrics(core_id);
            drop(mon);

            // Update gauges based on metric name
            for (metric_name, value) in metrics {
                let metric_enum = match metric_name.as_str() {
                    "IPC" => Some(CoreMetric::IPC),
                    "instructions" => Some(CoreMetric::Instructions),
                    "cycles" => Some(CoreMetric::Cycles),
                    "L3CacheMissNum" => Some(CoreMetric::L3CacheMiss),
                    "L3CacheRef" => Some(CoreMetric::L3CacheRef),
                    "L2CacheMissNum" => Some(CoreMetric::L2CacheMiss),
                    "L2CacheRef" => Some(CoreMetric::L2CacheRef),
                    "L3CacheHitRatio" => Some(CoreMetric::L3CacheHitRatio),
                    "L2CacheHitRatio" => Some(CoreMetric::L2CacheHitRatio),
                    "L3MPI" => Some(CoreMetric::L3MPI),
                    "L2MPI" => Some(CoreMetric::L2MPI),
                    "elapsedTime" => Some(CoreMetric::ElapsedTime),
                    "L2PrefetchMiss" => Some(CoreMetric::L2PrefetchMiss),
                    "L2PrefetchHit" => Some(CoreMetric::L2PrefetchHit),
                    "L2OutSilent" => Some(CoreMetric::L2OutSilent),
                    "L2OutNonSilent" => Some(CoreMetric::L2OutNonSilent),
                    "L2In" => Some(CoreMetric::L2In),
                    "L2Writeback" => Some(CoreMetric::L2Writeback),
                    _ => None,
                };

                if let Some(metric) = metric_enum {
                    if let Some(gauge) = self.core_gauges.get(&metric).and_then(|m| m.get(&core_id))
                    {
                        gauge.set(value);
                    }
                }
            }
        }
    }

    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }
}
