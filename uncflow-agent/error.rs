use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UncflowError {
    #[error("MSR operation failed: {0}")]
    MsrError(String),

    #[error("PCI operation failed: {0}")]
    PciError(String),

    #[error("Affinity operation failed: {0}")]
    AffinityError(String),

    #[error("RAPL operation failed: {0}")]
    RaplError(String),

    #[error("RDT operation failed: {0}")]
    RdtError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Nix error: {0}")]
    NixError(#[from] nix::Error),

    #[error("Prometheus error: {0}")]
    PrometheusError(#[from] prometheus::Error),

    #[error("Invalid hardware state: {0}")]
    HardwareError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unsupported architecture: {0}")]
    UnsupportedArchitecture(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}

pub type Result<T> = std::result::Result<T, UncflowError>;
