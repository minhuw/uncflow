// CHA Comprehensive Metrics Exporter
// Exports all 142 comprehensive CHA metrics

use prometheus::{Gauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::counters::cha::{ChaMonitor, LLCLookupType, LLCState, TransactionType};
use crate::error::Result;
use crate::metrics::cha::{ChaMetric, MetricCalculator, SFEvictionType, VictimType};

pub struct ChaMetricExporter {
    config: ExportConfig,
    registry: Arc<Registry>,
    monitor: Arc<parking_lot::Mutex<HashMap<i32, ChaMonitor>>>,
    socket_gauges: HashMap<ChaMetric, HashMap<i32, Gauge>>,
}

impl ChaMetricExporter {
    pub fn new(config: ExportConfig) -> Result<Self> {
        let registry = Arc::new(Registry::new());

        let mut monitors = HashMap::new();
        for &socket in &config.sockets {
            match ChaMonitor::new(socket) {
                Ok(mut monitor) => {
                    monitor.initialize()?;
                    monitors.insert(socket, monitor);
                    tracing::info!(
                        "Initialized comprehensive CHA monitor for socket {}",
                        socket
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize CHA for socket {}: {}", socket, e);
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
        let instance_label =
            std::env::var("INSTANCE_LABEL").unwrap_or_else(|_| "server".to_string());

        // Register all 142 CHA metrics
        for metric in ChaMetric::all() {
            let metric_name = metric.name();
            let opts = prometheus::Opts::new(
                metric_name.clone(),
                format!("CHA {metric_name} measurement"),
            );

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

        tracing::info!(
            "Registered {} CHA metrics for export",
            ChaMetric::all().len()
        );

        Ok(())
    }

    async fn collect_loop(
        config: ExportConfig,
        monitor: Arc<parking_lot::Mutex<HashMap<i32, ChaMonitor>>>,
        socket_gauges: HashMap<ChaMetric, HashMap<i32, Gauge>>,
    ) {
        tracing::info!("Starting comprehensive CHA export thread");

        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            for &socket_id in &config.sockets {
                let mut monitors = monitor.lock();

                if let Some(mon) = monitors.get_mut(&socket_id) {
                    // Collect raw event data
                    if let Ok(event_data) = mon.collect() {
                        drop(monitors);

                        // Create calculator with the event data
                        let mut calculator = MetricCalculator::new();
                        for (name, data) in event_data {
                            calculator.store_event(name, data);
                        }

                        // Calculate and export all transaction metrics
                        for trans_type in TransactionType::all() {
                            let metrics = calculator.calculate_transaction_metrics(trans_type);

                            for (metric_type, value) in metrics {
                                let metric = ChaMetric::Transaction(trans_type, metric_type);
                                if let Some(gauge) =
                                    socket_gauges.get(&metric).and_then(|m| m.get(&socket_id))
                                {
                                    gauge.set(value);
                                }
                            }
                        }

                        // Export LLC lookup metrics
                        for state in LLCState::all() {
                            for lookup_type in LLCLookupType::all() {
                                let value = calculator.get_llc_lookup(state, lookup_type);
                                let metric = ChaMetric::LLCLookup(state, lookup_type);
                                if let Some(gauge) =
                                    socket_gauges.get(&metric).and_then(|m| m.get(&socket_id))
                                {
                                    gauge.set(value as f64);
                                }
                            }
                        }

                        // Export LLC victim metrics
                        for victim_type in VictimType::all() {
                            let value = calculator.get_llc_victim(victim_type.name());
                            let metric = ChaMetric::LLCVictim(victim_type);
                            if let Some(gauge) =
                                socket_gauges.get(&metric).and_then(|m| m.get(&socket_id))
                            {
                                gauge.set(value as f64);
                            }
                        }

                        // Export SF eviction metrics
                        for eviction_type in SFEvictionType::all() {
                            let value = calculator.get_sf_eviction(eviction_type.name());
                            let metric = ChaMetric::SFEviction(eviction_type);
                            if let Some(gauge) =
                                socket_gauges.get(&metric).and_then(|m| m.get(&socket_id))
                            {
                                gauge.set(value as f64);
                            }
                        }

                        // Export eviction metrics
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::EvictionBandwidth)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.calculate_eviction_bandwidth());
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::EvictionLatency)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.calculate_eviction_latency());
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::EvictionQueueOccupancy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.calculate_eviction_queue_occupancy());
                        }

                        // Export queue occupancy
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::IRQOccupancy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.get_queue_occupancy("IRQ"));
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::PRQOccupancy)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.get_queue_occupancy("PRQ"));
                        }

                        // Export frequency
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::UncoreFrequency)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.calculate_uncore_frequency());
                        }

                        // Export credit metrics
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::ReadNoCredit)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.get_credit_metric("ReadNoCredit") as f64);
                        }
                        if let Some(gauge) = socket_gauges
                            .get(&ChaMetric::WriteNoCredit)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(calculator.get_credit_metric("WriteNoCredit") as f64);
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
                if let Ok(event_data) = mon.collect() {
                    drop(monitors);

                    let mut calculator = MetricCalculator::new();
                    for (name, data) in event_data {
                        calculator.store_event(name, data);
                    }

                    // Calculate and export all transaction metrics
                    for trans_type in TransactionType::all() {
                        let metrics = calculator.calculate_transaction_metrics(trans_type);

                        for (metric_type, value) in metrics {
                            let metric = ChaMetric::Transaction(trans_type, metric_type);
                            if let Some(gauge) = self
                                .socket_gauges
                                .get(&metric)
                                .and_then(|m| m.get(&socket_id))
                            {
                                gauge.set(value);
                            }
                        }
                    }

                    // Export LLC lookup metrics
                    for state in LLCState::all() {
                        for lookup_type in LLCLookupType::all() {
                            let value = calculator.get_llc_lookup(state, lookup_type);
                            let metric = ChaMetric::LLCLookup(state, lookup_type);
                            if let Some(gauge) = self
                                .socket_gauges
                                .get(&metric)
                                .and_then(|m| m.get(&socket_id))
                            {
                                gauge.set(value as f64);
                            }
                        }
                    }

                    // Export LLC victim metrics
                    for victim_type in VictimType::all() {
                        let value = calculator.get_llc_victim(victim_type.name());
                        let metric = ChaMetric::LLCVictim(victim_type);
                        if let Some(gauge) = self
                            .socket_gauges
                            .get(&metric)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(value as f64);
                        }
                    }

                    // Export SF eviction metrics
                    for eviction_type in SFEvictionType::all() {
                        let value = calculator.get_sf_eviction(eviction_type.name());
                        let metric = ChaMetric::SFEviction(eviction_type);
                        if let Some(gauge) = self
                            .socket_gauges
                            .get(&metric)
                            .and_then(|m| m.get(&socket_id))
                        {
                            gauge.set(value as f64);
                        }
                    }

                    // Export eviction metrics
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::EvictionBandwidth)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.calculate_eviction_bandwidth());
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::EvictionLatency)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.calculate_eviction_latency());
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::EvictionQueueOccupancy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.calculate_eviction_queue_occupancy());
                    }

                    // Export queue occupancy
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::IRQOccupancy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.get_queue_occupancy("IRQ"));
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::PRQOccupancy)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.get_queue_occupancy("PRQ"));
                    }

                    // Export frequency
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::UncoreFrequency)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.calculate_uncore_frequency());
                    }

                    // Export credit metrics
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::ReadNoCredit)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.get_credit_metric("ReadNoCredit") as f64);
                    }
                    if let Some(gauge) = self
                        .socket_gauges
                        .get(&ChaMetric::WriteNoCredit)
                        .and_then(|m| m.get(&socket_id))
                    {
                        gauge.set(calculator.get_credit_metric("WriteNoCredit") as f64);
                    }
                }
            }
        }
    }

    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }
}
