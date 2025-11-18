// Centralized metric collection orchestrator
// Manages all counter collection loops in a single unified async loop

use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::config::ExportConfig;
use crate::prom::{
    ChaMetricExporter, CoreMetricExporter, IioMetricExporter, ImcMetricExporter, IrpMetricExporter,
    RaplMetricExporter, RdtMetricExporter,
};

/// Configuration for which metrics to collect
#[derive(Debug, Clone, Default)]
pub struct CollectorConfig {
    pub rapl: bool,
    pub rdt: bool,
    pub core_metrics: bool,
    pub imc: bool,
    pub cha: bool,
    pub irp: bool,
    pub iio: bool,
}

/// Centralized collector that orchestrates all metric collection
pub struct MetricCollector {
    #[allow(dead_code)]
    config: ExportConfig,
    #[allow(dead_code)]
    collector_config: CollectorConfig,

    // Exporters (without their own loops)
    rapl_exporter: Option<Arc<RaplMetricExporter>>,
    rdt_exporter: Option<Arc<RdtMetricExporter>>,
    core_exporter: Option<Arc<CoreMetricExporter>>,
    imc_exporter: Option<Arc<ImcMetricExporter>>,
    cha_exporter: Option<Arc<ChaMetricExporter>>,
    irp_exporter: Option<Arc<IrpMetricExporter>>,
    iio_exporter: Option<Arc<IioMetricExporter>>,
}

impl MetricCollector {
    pub fn new(
        config: ExportConfig,
        collector_config: CollectorConfig,
    ) -> crate::error::Result<Self> {
        let mut collector = Self {
            config: config.clone(),
            collector_config: collector_config.clone(),
            rapl_exporter: None,
            rdt_exporter: None,
            core_exporter: None,
            imc_exporter: None,
            cha_exporter: None,
            irp_exporter: None,
            iio_exporter: None,
        };

        // Initialize exporters based on config using macro
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            rapl_exporter,
            rapl,
            RaplMetricExporter,
            "RAPL"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            rdt_exporter,
            rdt,
            RdtMetricExporter,
            "RDT"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            core_exporter,
            core_metrics,
            CoreMetricExporter,
            "Core PMU"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            imc_exporter,
            imc,
            ImcMetricExporter,
            "IMC"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            cha_exporter,
            cha,
            ChaMetricExporter,
            "CHA"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            irp_exporter,
            irp,
            IrpMetricExporter,
            "IRP"
        );
        crate::init_exporter!(
            collector,
            collector_config,
            config,
            iio_exporter,
            iio,
            IioMetricExporter,
            "IIO"
        );

        Ok(collector)
    }

    /// Start the centralized collection loop
    pub fn start(self) -> JoinHandle<()> {
        tracing::warn!("Starting centralized metric collection orchestrator");

        tokio::spawn(async move {
            self.collection_loop().await;
        })
    }

    /// Main unified collection loop
    async fn collection_loop(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Collect all metrics in parallel using macro
            let mut tasks = Vec::new();

            crate::spawn_collector!(tasks, &self.rapl_exporter);
            crate::spawn_collector!(tasks, &self.rdt_exporter);
            crate::spawn_collector!(tasks, &self.core_exporter);
            crate::spawn_collector!(tasks, &self.imc_exporter);
            crate::spawn_collector!(tasks, &self.cha_exporter);
            crate::spawn_collector!(tasks, &self.irp_exporter);
            crate::spawn_collector!(tasks, &self.iio_exporter);

            // Wait for all collections to complete
            for task in tasks {
                if let Err(e) = task.await {
                    tracing::error!("Collection task failed: {}", e);
                }
            }
        }
    }

    /// Get references to exporters for metrics handler
    pub fn rapl_exporter(&self) -> Option<Arc<RaplMetricExporter>> {
        self.rapl_exporter.clone()
    }

    pub fn rdt_exporter(&self) -> Option<Arc<RdtMetricExporter>> {
        self.rdt_exporter.clone()
    }

    pub fn core_exporter(&self) -> Option<Arc<CoreMetricExporter>> {
        self.core_exporter.clone()
    }

    pub fn imc_exporter(&self) -> Option<Arc<ImcMetricExporter>> {
        self.imc_exporter.clone()
    }

    pub fn cha_exporter(&self) -> Option<Arc<ChaMetricExporter>> {
        self.cha_exporter.clone()
    }

    pub fn irp_exporter(&self) -> Option<Arc<IrpMetricExporter>> {
        self.irp_exporter.clone()
    }

    pub fn iio_exporter(&self) -> Option<Arc<IioMetricExporter>> {
        self.iio_exporter.clone()
    }
}
