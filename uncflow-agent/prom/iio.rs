// IIO Metrics Exporter

use crate::counters::iio::IioMonitor;
use crate::error::Result;
use crate::metrics::iio::IioMetric;
use crate::ExportConfig;
use prometheus::{Gauge, Registry};
use std::collections::HashMap;

use std::thread;
use std::time::Duration;

pub struct IioMetricExporter {
    monitors: Vec<IioMonitor>,
    registry: Registry,
    gauges: HashMap<(i32, String), Gauge>,
}

impl IioMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Registry::new();
        let mut monitors = Vec::new();
        let mut gauges = HashMap::new();

        // Create monitors for each socket
        for &socket in &config.sockets {
            let monitor = IioMonitor::new(socket)?;
            monitors.push(monitor);

            // Register gauges for each metric on this socket
            for metric in IioMetric::all() {
                let metric_name = metric.name();
                let gauge = Gauge::new(
                    format!("iio_{socket}_{metric_name}"),
                    format!("IIO {metric_name} for socket {socket}"),
                )?;
                registry.register(Box::new(gauge.clone()))?;
                gauges.insert((socket, metric_name), gauge);
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
                if let Ok(mut monitor) = IioMonitor::new(socket) {
                    match monitor.collect_metrics() {
                        Ok(metrics) => {
                            for (metric, value) in metrics {
                                let metric_name = metric.name();
                                if let Some(gauge) = gauges.get(&(socket, metric_name)) {
                                    gauge.set(value);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to collect IIO metrics for socket {}: {}",
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
            if let Ok(mut monitor) = IioMonitor::new(socket) {
                match monitor.collect_metrics() {
                    Ok(metrics) => {
                        for (metric, value) in metrics {
                            let metric_name = metric.name();
                            if let Some(gauge) = self.gauges.get(&(socket, metric_name)) {
                                gauge.set(value);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to collect IIO metrics for socket {}: {}",
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
