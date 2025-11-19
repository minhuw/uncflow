# uncflow-raw

Hardware register definitions for Intel Uncore Performance Monitoring.

## Overview

`uncflow-raw` provides type-safe abstractions over MSR (Model-Specific Register) access and hardware-specific constants for Intel Uncore performance monitoring units.

This crate is the **hardware abstraction layer** for the uncflow project, separating architecture-specific details from business logic.

## Features

- **Type-safe register programming** - Structured register layouts with validation
- **Multiple architecture support** - Feature flags select target CPU
- **Zero runtime overhead** - All abstractions compile to raw MSR operations
- **Well-documented** - Each register documents its bit layout

## Architecture Support

| Architecture | Feature Flag | Status |
|--------------|--------------|--------|
| Skylake-SP | `skylake` (default) | ✅ Implemented |
| Cascade Lake-SP | `cascadelake` | 🚧 Coming soon |
| Ice Lake-SP | `icelake` | 🚧 Coming soon |

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
uncflow-raw = { version = "0.1", features = ["skylake"] }
```

Example:

```rust
use uncflow_raw::current_arch::iio;
use uncflow_raw::{write_msr, RegisterLayout};

// Create typed register
let ctrl = iio::IioCounterControl {
    event_select: 0x41,
    unit_mask: 0x20,
    enable: true,
    channel_mask: 0xFF,
    fc_mask: 0x07,
    ..Default::default()
};

// Validate before writing
ctrl.validate()?;

// Write to MSR
write_msr(0, iio::msr::IIO_UNIT_CTL0[0], ctrl.to_msr_value())?;
```

## Safety

MSR operations require:
- Root privileges or `CAP_SYS_RAWIO` capability
- `/dev/cpu/*/msr` device access
- Knowledge of valid MSR addresses for your CPU

Writing invalid values can cause system instability. Always use the provided register types and validation.

## License

MIT
