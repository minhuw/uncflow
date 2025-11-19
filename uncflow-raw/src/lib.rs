//! # uncflow-raw
//!
//! Hardware register definitions for Intel Uncore Performance Monitoring.
//!
//! This crate provides type-safe abstractions over MSR (Model-Specific Register)
//! access and hardware-specific constants for various Intel CPU architectures,
//! from Haswell (Broadwell), Skylake (Cascade Lake), Ice Lake (Sapphire Rapids),
//! to Emerald Rapids (Granite Rapids).
//!
//! ## Features
//!
//! Select the target architecture via feature flags:
//! - `skylake` (default) - Skylake-SP register definitions
//! - `cascadelake` - Cascade Lake-SP register definitions
//! - `icelake` - Ice Lake-SP register definitions
//!
//! ## Usage
//!
//! ```ignore
//! use uncflow_raw::current_arch::iio;
//! use uncflow_raw::{read_msr, write_msr};
//!
//! // Use architecture-specific constants
//! let msr_addr = iio::msr::IIO_UNIT_CTL0[0];
//!
//! // Type-safe register programming
//! let ctrl = iio::IioCounterControl {
//!     event_select: 0x41,
//!     enable: true,
//!     ..Default::default()
//! };
//!
//! write_msr(0, msr_addr, ctrl.to_msr_value())?;
//! ```

pub mod arch;
pub mod msr;
pub mod register;

// Re-export for convenience
pub use msr::{read_msr, write_msr, MsrError, Result};
pub use register::{Register, RegisterLayout};

// Export current architecture based on feature flag
#[cfg(feature = "skylake")]
pub use arch::skylake as current_arch;

// Cascade Lake and Ice Lake are not yet implemented
// #[cfg(feature = "cascadelake")]
// pub use arch::cascadelake as current_arch;

// #[cfg(feature = "icelake")]
// pub use arch::icelake as current_arch;
