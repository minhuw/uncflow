# Orchestrator Module

The orchestrator module provides centralized management of all metric collection loops.

## Architecture

Instead of each exporter running its own independent collection loop, the orchestrator:
1. Creates all exporters without starting their loops
2. Runs a single unified async loop
3. Calls `collect()` on each exporter at coordinated intervals
4. Provides better control over scheduling and lifecycle

## Usage Example

```rust
use uncflow::{CollectorConfig, ExportConfig, MetricCollector};

#[tokio::main]
async fn main() -> Result<()> {
    // Configure which hardware to monitor
    let export_config = ExportConfig::auto_detect();
    
    // Configure which metrics to collect
    let collector_config = CollectorConfig {
        rapl: true,
        rdt: true,
        core_metrics: true,
        imc: true,
        cha: true,
        irp: false,
        iio: false,
    };
    
    // Create the centralized collector
    let collector = MetricCollector::new(export_config, collector_config)?;
    
    // Start the unified collection loop
    let _handle = collector.start();
    
    // ... run HTTP server for /metrics endpoint ...
    
    Ok(())
}
```

## Benefits

- **Unified scheduling**: All counters collected at the same intervals
- **Better coordination**: Easy to implement cross-counter correlations
- **Simplified lifecycle**: Start/stop all collectors together
- **Resource efficiency**: Single async loop instead of 7+ independent threads
- **Event rotation**: Future support for rotating between event sets

## Current Status

✅ Infrastructure in place
✅ All exporters have `collect()` methods
⚠️  `collect()` methods are stubs - need to extract logic from existing loops (TODO)

## Migration Path

The old API still works (each exporter has a `.start()` method). The orchestrator is an alternative approach that provides better control and coordination.
