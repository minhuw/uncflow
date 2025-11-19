use axum::{response::IntoResponse, routing::get, Router};
use clap::Parser;
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio_util::sync::CancellationToken;

use uncflow::{
    ChaMetricExporter, CollectorConfig, CoreMetricExporter, ExportConfig, IioMetricExporter,
    ImcMetricExporter, IrpMetricExporter, MetricCollector, RaplMetricExporter, RdtMetricExporter,
    Result,
};

#[derive(Parser, Debug)]
#[command(name = "uncflow")]
#[command(about = "Hardware performance monitoring for Intel CPUs")]
struct Args {
    #[arg(long, help = "Enable core metrics")]
    core_metrics: bool,

    #[arg(long, help = "Enable all uncore metrics (IMC, CHA, IRP, IIO)")]
    uncore: bool,

    #[arg(long, help = "Enable IMC (Integrated Memory Controller) metrics")]
    imc: bool,

    #[arg(
        long,
        help = "Enable CHA (Cache Agent/Home Agent) comprehensive metrics (142 metrics)"
    )]
    cha: bool,

    #[arg(long, help = "Enable IRP (IO Request Processing) metrics")]
    irp: bool,

    #[arg(long, help = "Enable IIO (Integrated IO) metrics")]
    iio: bool,

    #[arg(long, help = "Enable Intel RDT metrics (MBM)")]
    rdt: bool,

    #[arg(long, help = "Enable RAPL power/energy metrics")]
    rapl: bool,

    #[arg(
        long = "socket",
        help = "Sockets to monitor (can be specified multiple times, supports ranges: --socket 0 --socket 1 or --socket 0-1)",
        action = clap::ArgAction::Append
    )]
    sockets: Vec<String>,

    #[arg(
        long = "core",
        help = "Cores to monitor (supports ranges and comma-separated lists: --core 0-3,5-8 or --core 0-3 --core 5-8)",
        action = clap::ArgAction::Append
    )]
    cores: Vec<String>,

    #[arg(
        short,
        long,
        help = "Enable verbose logging (shows all MSR/PCI read/write operations)"
    )]
    verbose: bool,
}

struct AppState {
    rapl_exporter: Option<Arc<RaplMetricExporter>>,
    rdt_exporter: Option<Arc<RdtMetricExporter>>,
    core_exporter: Option<Arc<CoreMetricExporter>>,
    imc_exporter: Option<Arc<ImcMetricExporter>>,
    cha_exporter: Option<Arc<ChaMetricExporter>>,
    irp_exporter: Option<Arc<IrpMetricExporter>>,
    iio_exporter: Option<Arc<IioMetricExporter>>,
    collection_handle: Option<tokio::task::JoinHandle<()>>,
}

async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();

    // Gather metrics from all exporters using macro
    uncflow::gather_metrics!(buffer, encoder, state.rapl_exporter, "RAPL");
    uncflow::gather_metrics!(buffer, encoder, state.rdt_exporter, "RDT");
    uncflow::gather_metrics!(buffer, encoder, state.core_exporter, "Core");
    uncflow::gather_metrics!(buffer, encoder, state.imc_exporter, "IMC");
    uncflow::gather_metrics!(buffer, encoder, state.cha_exporter, "CHA");
    uncflow::gather_metrics!(buffer, encoder, state.irp_exporter, "IRP");
    uncflow::gather_metrics!(buffer, encoder, state.iio_exporter, "IIO");

    let content_type = encoder.format_type().to_string();
    (
        [("Content-Type", content_type)],
        String::from_utf8(buffer).unwrap_or_default(),
    )
}

fn check_permissions() {
    // Check if we can access MSR
    let msr_path = "/dev/cpu/0/msr";
    if std::fs::metadata(msr_path).is_err() {
        eprintln!("\n⚠️  ERROR: Cannot access {msr_path}\n\nThe MSR kernel module may not be loaded.\nRun: sudo modprobe msr\n");
        std::process::exit(1);
    }

    // Try to open MSR to check actual permissions
    if let Err(e) = std::fs::File::open(msr_path) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            eprintln!("\n⚠️  ERROR: Permission denied accessing {msr_path}\n\nRun with: cargo run --release -- --rapl\n(sudo is configured in .cargo/config.toml)\n");
            std::process::exit(1);
        }
    }
}

/// Parse a list of range strings like ["0-3", "5", "8-11"] into Vec<i32>
/// Supports multiple formats:
/// - Single values: "0", "5"
/// - Ranges: "0-3" (inclusive)
/// - Comma-separated: "0,2,4"
/// - Mixed: "0-3,5,8-11"
fn parse_range_list(inputs: &[String]) -> Vec<i32> {
    let mut result = Vec::new();

    for input in inputs {
        // Split by commas first
        for part in input.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Check if it's a range (contains '-')
            if let Some((start_str, end_str)) = part.split_once('-') {
                // Parse range
                if let (Ok(start), Ok(end)) = (
                    start_str.trim().parse::<i32>(),
                    end_str.trim().parse::<i32>(),
                ) {
                    result.extend(start..=end);
                } else {
                    tracing::warn!("Failed to parse range: {}", part);
                }
            } else {
                // Parse single value
                if let Ok(val) = part.parse::<i32>() {
                    result.push(val);
                } else {
                    tracing::warn!("Failed to parse value: {}", part);
                }
            }
        }
    }

    // Remove duplicates and sort
    result.sort_unstable();
    result.dedup();

    if result.is_empty() {
        tracing::warn!("No valid CPU/socket IDs parsed, using default: 0");
        result.push(0);
    }

    result
}

/// Initialize orchestrator mode (unified collection loop)
fn init_orchestrator_mode(
    config: ExportConfig,
    collector_config: CollectorConfig,
    cancel_token: CancellationToken,
) -> Result<AppState> {
    let collector = MetricCollector::new(config, collector_config)?;

    // Extract exporters for metrics handler BEFORE starting (which consumes self)
    let rapl_exporter = collector.rapl_exporter();
    let rdt_exporter = collector.rdt_exporter();
    let core_exporter = collector.core_exporter();
    let imc_exporter = collector.imc_exporter();
    let cha_exporter = collector.cha_exporter();
    let irp_exporter = collector.irp_exporter();
    let iio_exporter = collector.iio_exporter();

    // Start the unified collection loop with cancellation support (consumes collector)
    let collection_handle = collector.start(cancel_token);

    let state = AppState {
        rapl_exporter,
        rdt_exporter,
        core_exporter,
        imc_exporter,
        cha_exporter,
        irp_exporter,
        iio_exporter,
        collection_handle: Some(collection_handle),
    };

    Ok(state)
}

async fn shutdown_signal(cancel_token: CancellationToken) {
    tracing::info!("Installing signal handlers...");

    let ctrl_c = async {
        tracing::debug!("Waiting for Ctrl+C...");
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("Ctrl+C received!");
    };

    #[cfg(unix)]
    let terminate = async {
        tracing::debug!("Waiting for SIGTERM...");
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
        tracing::info!("SIGTERM received!");
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::warn!("Shutdown triggered by Ctrl+C");
        },
        _ = terminate => {
            tracing::warn!("Shutdown triggered by SIGTERM");
        },
    }

    tracing::warn!("Shutdown signal received, initiating graceful shutdown...");
    cancel_token.cancel();
    tracing::warn!("Cancellation token activated");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging based on verbose flag
    let log_level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt().with_max_level(log_level).init();

    // Check for root/capabilities early
    check_permissions();

    // Log detected architecture
    tracing::info!(
        "Detected CPU architecture: {}",
        uncflow::common::CPU_ARCH.name()
    );

    // Build configuration from CLI arguments
    let config = if args.sockets.is_empty() && args.cores.is_empty() {
        tracing::info!("Auto-detecting CPUs...");
        ExportConfig::auto_detect()
    } else {
        // Parse cores first (if specified)
        let cores = if !args.cores.is_empty() {
            parse_range_list(&args.cores)
        } else {
            // Default: detect all online cores if not specified
            ExportConfig::detect_online_cpus()
        };

        // Parse sockets
        let sockets = if !args.sockets.is_empty() {
            parse_range_list(&args.sockets)
        } else {
            // If cores specified but no sockets, auto-detect sockets from cores
            ExportConfig::detect_sockets(&cores)
        };

        tracing::info!("Using sockets: {:?}", sockets);
        tracing::info!("Using cores: {:?}", cores);

        ExportConfig::new(sockets, cores)
    };

    tracing::info!(
        "Monitoring {} sockets, {} cores",
        config.sockets.len(),
        config.cores.len()
    );

    // Determine which metrics to collect
    // Default: iio, imc, irp if no flags specified
    let no_flags_specified = !args.rapl
        && !args.rdt
        && !args.core_metrics
        && !args.uncore
        && !args.imc
        && !args.cha
        && !args.irp
        && !args.iio;

    let collector_config = CollectorConfig {
        rapl: args.rapl,
        rdt: args.rdt,
        core_metrics: args.core_metrics,
        imc: args.uncore || args.imc || no_flags_specified,
        cha: args.uncore || args.cha,
        irp: args.uncore || args.irp || no_flags_specified,
        iio: args.uncore || args.iio || no_flags_specified,
    };

    if no_flags_specified {
        tracing::info!("No metrics specified, using defaults: IIO, IMC, IRP");
    }

    let cancel_token = CancellationToken::new();

    tracing::info!("Using orchestrator mode (unified collection loop)");
    let mut state = init_orchestrator_mode(config, collector_config, cancel_token.clone())?;

    let collection_handle = state.collection_handle.take();

    let app_state = Arc::new(state);

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::warn!("Starting HTTP server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_token))
        .await?;

    tracing::info!("Server shutdown complete, waiting for collection loop to finish...");

    if let Some(handle) = collection_handle {
        let _ = handle.await;
    }

    tracing::info!("All tasks completed, exiting");

    Ok(())
}
