use prometheus::{Gauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::counters::imc::ImcMonitor;
use crate::error::Result;
use crate::metrics::imc::ImcMetric;

pub struct ImcMetricExporter {
    config: ExportConfig,
    registry: Arc<Registry>,
    monitor: Arc<parking_lot::Mutex<HashMap<i32, ImcMonitor>>>,
    socket_gauges: HashMap<ImcMetric, HashMap<i32, Gauge>>,
}

impl ImcMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());

        let mut monitors = HashMap::new();
        for &socket in &config.sockets {
            match ImcMonitor::new(socket) {
                Ok(mut monitor) => {
                    monitor.initialize()?;
                    monitors.insert(socket, monitor);
                    tracing::info!("Initialized IMC monitor for socket {}", socket);
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize IMC for socket {}: {}", socket, e);
                }
            }
        }

        let monitor = Arc::new(parking_lot::Mutex::new(monitors));

        let mut exporter = Self {
            config: config.clone(),
            registry: Arc::clone(&registry),
            monitor,
            socket_gauges: HashMap::new(),
        };

        exporter.register_metrics()?;

        Ok(exporter)
    }

    fn register_metrics(&mut self) -> Result<()> {
        let instance_label = std::env::var("INSTANCE_LABEL").unwrap_or_else(|_| "none".to_string());

        for metric in ImcMetric::all() {
            let opts =
                prometheus::Opts::new(metric.name(), format!("IMC {} measurement", metric.name()));

            let mut socket_map = HashMap::new();
            for &socket_id in &self.config.sockets {
                let gauge = Gauge::with_opts(
                    opts.clone()
                        .const_label("socket", socket_id.to_string())
                        .const_label("instance", &instance_label),
                )?;
                self.registry.register(Box::new(gauge.clone()))?;
                socket_map.insert(socket_id, gauge);
            }
            self.socket_gauges.insert(metric, socket_map);
        }

        Ok(())
    }

    async fn collect_loop(
        config: ExportConfig,
        monitor: Arc<parking_lot::Mutex<HashMap<i32, ImcMonitor>>>,
        socket_gauges: HashMap<ImcMetric, HashMap<i32, Gauge>>,
    ) {
        tracing::info!("Starting IMC export thread");

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            for &socket_id in &config.sockets {
                let mut monitors = monitor.lock();

                if let Some(mon) = monitors.get_mut(&socket_id) {
                    if let Ok(metrics) = mon.collect() {
                        drop(monitors);

                        // Update bandwidth gauges
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryReadBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.read_bandwidth as f64);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryWriteBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.write_bandwidth as f64);
                        }

                        // Update latency gauges
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryReadLatency)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.read_latency);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryWriteLatency)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.write_latency);
                        }

                        // Update queue occupancy gauges
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryRPQOccupancy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.rpq_occupancy as f64);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryWPQOccupancy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.wpq_occupancy as f64);
                        }

                        // Update queue status gauges (new metrics)
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::IMCRPQNonEmpty)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.rpq_non_empty);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::IMCRPQFull)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.rpq_full);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::IMCWPQNonEmpty)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.wpq_non_empty);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::IMCWPQFull)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.wpq_full);
                        }

                        // Update frequency gauge
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::IMCFrequency)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.frequency);
                        }

                        // NUMA metrics (placeholder - require topology info)
                        // For now, assume all traffic is local
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryLocalReadBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.read_bandwidth as f64); // All local for now
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryLocalWriteBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(metrics.write_bandwidth as f64); // All local for now
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryRemoteReadBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(0.0); // No remote for now
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryRemoteWriteBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(0.0); // No remote for now
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryLocalReadRatio)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(1.0); // 100% local for now
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ImcMetric::MemoryLocalWriteRatio)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(1.0); // 100% local for now
                        }
                    } else {
                        drop(monitors);
                    }
                }
            }
        }
    }

    pub fn start(&self) -> JoinHandle<()> {
        let config = self.config.clone();
        let monitor = Arc::clone(&self.monitor);
        let socket_gauges = self.socket_gauges.clone();

        tokio::spawn(Self::collect_loop(config, monitor, socket_gauges))
    }

    /// Collect metrics once (called by orchestrator)
    pub async fn collect(&self) {
        for &socket_id in &self.config.sockets {
            let mut monitors = self.monitor.lock();

            if let Some(mon) = monitors.get_mut(&socket_id) {
                if let Ok(metrics) = mon.collect() {
                    drop(monitors);

                    // Update bandwidth gauges
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryReadBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.read_bandwidth as f64);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryWriteBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.write_bandwidth as f64);
                    }

                    // Update latency gauges
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryReadLatency)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.read_latency);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryWriteLatency)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.write_latency);
                    }

                    // Update queue occupancy gauges
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryRPQOccupancy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.rpq_occupancy as f64);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryWPQOccupancy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.wpq_occupancy as f64);
                    }

                    // Update queue status gauges
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::IMCRPQNonEmpty)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.rpq_non_empty);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::IMCRPQFull)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.rpq_full);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::IMCWPQNonEmpty)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.wpq_non_empty);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::IMCWPQFull)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.wpq_full);
                    }

                    // Update frequency gauge
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::IMCFrequency)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.frequency);
                    }

                    // NUMA metrics
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryLocalReadBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.read_bandwidth as f64);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryLocalWriteBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(metrics.write_bandwidth as f64);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryRemoteReadBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(0.0);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryRemoteWriteBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(0.0);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryLocalReadRatio)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(1.0);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ImcMetric::MemoryLocalWriteRatio)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(1.0);
                    }
                }
            }
        }
    }

    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }
}
