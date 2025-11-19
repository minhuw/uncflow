//! Intel Skylake-SP (Skylake Server) register definitions
//!
//! This module provides hardware register definitions for Intel Xeon Scalable
//! processors based on the Skylake microarchitecture (Skylake-SP).
//!
//! ## Uncore Units
//!
//! - **CHA** (Caching/Home Agent) - LLC cache and snoop filter
//! - **IIO** (Integrated I/O) - PCIe root complex
//! - **IMC** (Integrated Memory Controller) - DDR4 memory controller
//! - **IRP** (I/O Request Processing) - I/O arbitration
//! - **RAPL** (Running Average Power Limit) - Power monitoring
//! - **RDT** (Resource Director Technology) - Cache/memory monitoring
//! - **Core** - Core performance monitoring units
//!
//! ## References
//!
//! - Intel® Xeon® Processor Scalable Family Specification Update
//! - Intel® 64 and IA-32 Architectures Software Developer's Manual, Volume 3B

pub mod cha;
pub mod core;
pub mod iio;
pub mod imc;
pub mod irp;
pub mod rapl;
pub mod rdt;
