use prometheus::{Gauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::counters::rapl::RaplMonitor;
use crate::error::Result;
use crate::metrics::rapl::RaplMetric;

pub struct RaplMetricExporter {
    config: ExportConfig,
    registry: Arc<Registry>,
    monitor: Arc<parking_lot::Mutex<RaplMonitor>>,
    socket_gauges: HashMap<RaplMetric, HashMap<i32, Gauge>>,
}

impl RaplMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());
        let monitor = Arc::new(parking_lot::Mutex::new(RaplMonitor::new(config.clone())?));

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
        for metric in RaplMetric::all() {
            let opts =
                prometheus::Opts::new(metric.name(), format!("RAPL {} measurement", metric.name()));

            let mut socket_map = HashMap::new();
            for &socket_id in &self.config.sockets {
                let gauge =
                    Gauge::with_opts(opts.clone().const_label("socket", socket_id.to_string()))?;
                self.registry.register(Box::new(gauge.clone()))?;
                socket_map.insert(socket_id, gauge);
            }
            self.socket_gauges.insert(metric, socket_map);
        }

        Ok(())
    }

    /// Collect metrics once (called by orchestrator)
    pub async fn collect(&self) {
        for &socket_id in &self.config.sockets {
            let mut monitor = self.monitor.lock();

            match monitor.get_current_energy(socket_id) {
                Ok(energy_data) => {
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::PackageEnergy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(energy_data.package_energy);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::CoreEnergy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(energy_data.core_energy);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::DramEnergy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(energy_data.dram_energy);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get energy data for socket {}: {}", socket_id, e);
                }
            }

            match monitor.get_power_consumption(socket_id) {
                Ok(power_data) => {
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::PackagePower)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(power_data.package_energy);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::CorePower)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(power_data.core_energy);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&RaplMetric::DramPower)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(power_data.dram_energy);
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to get power consumption for socket {}: {}",
                        socket_id,
                        e
                    );
                }
            }
        }
    }

    async fn collect_loop(
        config: ExportConfig,
        monitor: Arc<parking_lot::Mutex<RaplMonitor>>,
        socket_gauges: HashMap<RaplMetric, HashMap<i32, Gauge>>,
    ) {
        tracing::warn!("Starting RAPL export thread");

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            for &socket_id in &config.sockets {
                let mut monitor = monitor.lock();

                match monitor.get_current_energy(socket_id) {
                    Ok(energy_data) => {
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::PackageEnergy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(energy_data.package_energy);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::CoreEnergy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(energy_data.core_energy);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::DramEnergy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(energy_data.dram_energy);
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to get energy data for socket {}: {}",
                            socket_id,
                            e
                        );
                    }
                }

                match monitor.get_power_consumption(socket_id) {
                    Ok(power_data) => {
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::PackagePower)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(power_data.package_energy);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::CorePower)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(power_data.core_energy);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&RaplMetric::DramPower)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(power_data.dram_energy);
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to get power consumption for socket {}: {}",
                            socket_id,
                            e
                        );
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

    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }
}
