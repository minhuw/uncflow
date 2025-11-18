// Macros (must be first for visibility)
#[macro_use]
pub mod macros;

pub mod common;
pub mod config;
pub mod counters;
pub mod error;
pub mod metrics;
pub mod orchestrator;
pub mod prom;

pub use config::ExportConfig;
pub use error::{Result, UncflowError};
pub use orchestrator::{CollectorConfig, MetricCollector};

// Re-export for backward compatibility
pub use prom::{
    ChaMetricExporter, CoreMetricExporter, IioMetricExporter, ImcMetricExporter, IrpMetricExporter,
    RaplMetricExporter, RdtMetricExporter,
};
