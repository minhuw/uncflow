use prometheus::{Gauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::counters::rdt::RdtMonitor;
use crate::error::Result;
use crate::metrics::rdt::RdtMetric;

pub struct RdtMetricExporter {
    config: ExportConfig,
    registry: Arc<Registry>,
    monitor: Arc<parking_lot::Mutex<RdtMonitor>>,
    socket_gauges: HashMap<RdtMetric, HashMap<i32, Gauge>>,
    core_gauges: HashMap<RdtMetric, HashMap<i32, Gauge>>,
    rmid_refresh_counter: Arc<parking_lot::Mutex<u32>>,
}

impl RdtMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());

        let mut monitor = RdtMonitor::new(config.clone())?;
        monitor.initialize()?;

        let monitor = Arc::new(parking_lot::Mutex::new(monitor));

        let mut exporter = Self {
            config: config.clone(),
            registry: Arc::clone(&registry),
            monitor,
            socket_gauges: HashMap::new(),
            core_gauges: HashMap::new(),
            rmid_refresh_counter: Arc::new(parking_lot::Mutex::new(0)),
        };

        exporter.register_metrics()?;

        Ok(exporter)
    }

    fn register_metrics(&mut self) -> Result<()> {
        for metric in RdtMetric::all() {
            let opts =
                prometheus::Opts::new(metric.name(), format!("RDT {} measurement", metric.name()));

            let mut socket_map = HashMap::new();
            for &socket_id in &self.config.sockets {
                let gauge =
                    Gauge::with_opts(opts.clone().const_label("socket", socket_id.to_string()))?;
                self.registry.register(Box::new(gauge.clone()))?;
                socket_map.insert(socket_id, gauge);
            }
            self.socket_gauges.insert(metric, socket_map);

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
        monitor: Arc<parking_lot::Mutex<RdtMonitor>>,
        socket_gauges: HashMap<RdtMetric, HashMap<i32, Gauge>>,
        core_gauges: HashMap<RdtMetric, HashMap<i32, Gauge>>,
    ) {
        tracing::warn!("Starting RDT export thread");

        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut rmid_refresh_counter = 0u32;

        loop {
            interval.tick().await;

            {
                let mut mon = monitor.lock();
                if let Err(e) = mon.update() {
                    tracing::error!("Failed to update RDT metrics: {}", e);
                    continue;
                }
            }

            // Update socket-level gauges
            for &socket_id in &config.sockets {
                let mon = monitor.lock();
                let socket_metrics = mon.get_socket_metrics(socket_id);
                drop(mon);

                if let Some(gauge) = socket_gauges
                    .get(&RdtMetric::LocalMemoryBandwidth)
                    .and_then(|m| m.get(&socket_id))
                {
                    if let Some(&value) = socket_metrics.get("LocalMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = socket_gauges
                    .get(&RdtMetric::RemoteMemoryBandwidth)
                    .and_then(|m| m.get(&socket_id))
                {
                    if let Some(&value) = socket_metrics.get("RemoteMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = socket_gauges
                    .get(&RdtMetric::TotalMemoryBandwidth)
                    .and_then(|m| m.get(&socket_id))
                {
                    if let Some(&value) = socket_metrics.get("TotalMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = socket_gauges
                    .get(&RdtMetric::LlcOccupancy)
                    .and_then(|m| m.get(&socket_id))
                {
                    if let Some(&value) = socket_metrics.get("CMTLLCOccupancy") {
                        gauge.set(value);
                    }
                }
            }

            // Update per-core gauges
            for &core_id in &config.cores {
                let mon = monitor.lock();
                let metrics = mon.get_metrics(core_id);
                drop(mon);

                if let Some(gauge) = core_gauges
                    .get(&RdtMetric::LocalMemoryBandwidth)
                    .and_then(|m| m.get(&core_id))
                {
                    if let Some(&value) = metrics.get("LocalMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = core_gauges
                    .get(&RdtMetric::RemoteMemoryBandwidth)
                    .and_then(|m| m.get(&core_id))
                {
                    if let Some(&value) = metrics.get("RemoteMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = core_gauges
                    .get(&RdtMetric::TotalMemoryBandwidth)
                    .and_then(|m| m.get(&core_id))
                {
                    if let Some(&value) = metrics.get("TotalMemoryBandwidth") {
                        gauge.set(value);
                    }
                }

                if let Some(gauge) = core_gauges
                    .get(&RdtMetric::LlcOccupancy)
                    .and_then(|m| m.get(&core_id))
                {
                    if let Some(&value) = metrics.get("CMTLLCOccupancy") {
                        gauge.set(value);
                    }
                }
            }

            rmid_refresh_counter += 1;
            if rmid_refresh_counter >= 30 {
                let mut mon = monitor.lock();
                if let Err(e) = mon.refresh_rmids() {
                    tracing::error!("Failed to refresh RMIDs: {}", e);
                }
                rmid_refresh_counter = 0;
            }
        }
    }

    pub fn start(&self) -> JoinHandle<()> {
        let config = self.config.clone();
        let monitor = Arc::clone(&self.monitor);
        let socket_gauges = self.socket_gauges.clone();
        let core_gauges = self.core_gauges.clone();

        tokio::spawn(Self::collect_loop(
            config,
            monitor,
            socket_gauges,
            core_gauges,
        ))
    }

    /// Collect metrics once (called by orchestrator)
    pub async fn collect(&self) {
        {
            let mut mon = self.monitor.lock();
            if let Err(e) = mon.update() {
                tracing::error!("Failed to update RDT metrics: {}", e);
                return;
            }
        }

        // Update socket-level gauges
        for &socket_id in &self.config.sockets {
            let mon = self.monitor.lock();
            let socket_metrics = mon.get_socket_metrics(socket_id);
            drop(mon);

            if let Some(gauge) = self
                .socket_gauges
                .get(&RdtMetric::LocalMemoryBandwidth)
                .and_then(|m| m.get(&socket_id))
            {
                if let Some(&value) = socket_metrics.get("LocalMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .socket_gauges
                .get(&RdtMetric::RemoteMemoryBandwidth)
                .and_then(|m| m.get(&socket_id))
            {
                if let Some(&value) = socket_metrics.get("RemoteMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .socket_gauges
                .get(&RdtMetric::TotalMemoryBandwidth)
                .and_then(|m| m.get(&socket_id))
            {
                if let Some(&value) = socket_metrics.get("TotalMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .socket_gauges
                .get(&RdtMetric::LlcOccupancy)
                .and_then(|m| m.get(&socket_id))
            {
                if let Some(&value) = socket_metrics.get("CMTLLCOccupancy") {
                    gauge.set(value);
                }
            }
        }

        // Update per-core gauges
        for &core_id in &self.config.cores {
            let mon = self.monitor.lock();
            let metrics = mon.get_metrics(core_id);
            drop(mon);

            if let Some(gauge) = self
                .core_gauges
                .get(&RdtMetric::LocalMemoryBandwidth)
                .and_then(|m| m.get(&core_id))
            {
                if let Some(&value) = metrics.get("LocalMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .core_gauges
                .get(&RdtMetric::RemoteMemoryBandwidth)
                .and_then(|m| m.get(&core_id))
            {
                if let Some(&value) = metrics.get("RemoteMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .core_gauges
                .get(&RdtMetric::TotalMemoryBandwidth)
                .and_then(|m| m.get(&core_id))
            {
                if let Some(&value) = metrics.get("TotalMemoryBandwidth") {
                    gauge.set(value);
                }
            }

            if let Some(gauge) = self
                .core_gauges
                .get(&RdtMetric::LlcOccupancy)
                .and_then(|m| m.get(&core_id))
            {
                if let Some(&value) = metrics.get("CMTLLCOccupancy") {
                    gauge.set(value);
                }
            }
        }

        // Handle RMID refresh every 30 collections
        let mut counter = self.rmid_refresh_counter.lock();
        *counter += 1;
        if *counter >= 30 {
            let mut mon = self.monitor.lock();
            if let Err(e) = mon.refresh_rmids() {
                tracing::error!("Failed to refresh RMIDs: {}", e);
            }
            *counter = 0;
        }
    }

    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }
}
