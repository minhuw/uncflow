//! Architecture-specific register definitions
//!
//! Each Intel CPU architecture has different MSR addresses, register layouts,
//! and uncore unit configurations. This module provides architecture-specific
//! definitions organized by CPU family.
//!
//! ## Supported Architectures
//!
//! - **Skylake-SP** (`skylake` feature) - Intel Xeon Scalable (Skylake Server)
//! - Cascade Lake-SP (`cascadelake` feature) - Coming soon
//! - Ice Lake-SP (`icelake` feature) - Coming soon

#[cfg(feature = "skylake")]
pub mod skylake;

// Cascade Lake and Ice Lake are not yet implemented
// #[cfg(feature = "cascadelake")]
// pub mod cascadelake;

// #[cfg(feature = "icelake")]
// pub mod icelake;
