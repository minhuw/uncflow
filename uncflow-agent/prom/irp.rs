// IRP Metrics Exporter

use crate::counters::irp::IrpMonitor;
use crate::error::Result;
use crate::metrics::irp::IrpMetric;
use crate::ExportConfig;
use prometheus::{Gauge, Registry};
use std::collections::HashMap;

use std::thread;
use std::time::Duration;

pub struct IrpMetricExporter {
    monitors: Vec<IrpMonitor>,
    registry: Registry,
    gauges: HashMap<(i32, IrpMetric), Gauge>,
}

impl IrpMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Registry::new();
        let mut monitors = Vec::new();
        let mut gauges = HashMap::new();

        // Create monitors for each socket
        for &socket in &config.sockets {
            let monitor = IrpMonitor::new(socket)?;
            monitors.push(monitor);
        }

        // Register gauges for each metric and socket combination
        // Each metric needs to be registered only once as a metric family
        // with socket as a label dimension
        for metric in IrpMetric::all() {
            for &socket in &config.sockets {
                let gauge = Gauge::with_opts(
                    prometheus::Opts::new(metric.name(), format!("IRP {} metric", metric.name()))
                        .const_label("socket", socket.to_string()),
                )?;
                registry.register(Box::new(gauge.clone()))?;
                gauges.insert((socket, metric), gauge);
            }
        }

        Ok(Self {
            monitors,
            registry,
            gauges,
        })
    }

    pub fn start(&self) {
        let monitors = self.monitors.iter().map(|m| m.socket()).collect::<Vec<_>>();
        let gauges = self.gauges.clone();

        thread::spawn(move || loop {
            for &socket in &monitors {
                if let Ok(mut monitor) = IrpMonitor::new(socket) {
                    match monitor.collect_metrics() {
                        Ok(metrics) => {
                            for (metric, value) in metrics {
                                if let Some(gauge) = gauges.get(&(socket, metric)) {
                                    gauge.set(value);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to collect IRP metrics for socket {}: {}",
                                socket,
                                e
                            );
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        });
    }

    /// Collect metrics once (called by orchestrator)
    pub async fn collect(&self) {
        let sockets: Vec<_> = self.monitors.iter().map(|m| m.socket()).collect();

        for &socket in &sockets {
            if let Ok(mut monitor) = IrpMonitor::new(socket) {
                match monitor.collect_metrics() {
                    Ok(metrics) => {
                        for (metric, value) in metrics {
                            if let Some(gauge) = self.gauges.get(&(socket, metric)) {
                                gauge.set(value);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to collect IRP metrics for socket {}: {}",
                            socket,
                            e
                        );
                    }
                }
            }
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}
