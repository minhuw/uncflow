// CPU Architecture detection and configuration

use once_cell::sync::Lazy;

use crate::common::cpuid;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArchitecture {
    Skylake,
    Haswell,
    Broadwell,
    CascadeLake,
    IceLake,
    Unknown,
}

impl CpuArchitecture {
    pub fn name(&self) -> &'static str {
        match self {
            CpuArchitecture::Skylake => "Skylake",
            CpuArchitecture::Haswell => "Haswell",
            CpuArchitecture::Broadwell => "Broadwell",
            CpuArchitecture::CascadeLake => "Cascade Lake",
            CpuArchitecture::IceLake => "Ice Lake",
            CpuArchitecture::Unknown => "Unknown",
        }
    }
}

pub static CPU_ARCH: Lazy<CpuArchitecture> =
    Lazy::new(|| detect_architecture().unwrap_or(CpuArchitecture::Unknown));

fn detect_architecture() -> Result<CpuArchitecture> {
    // CPUID leaf 1: Family, Model, Stepping
    let (eax, _ebx, _ecx, _edx) = cpuid::cpuid(1, 0);

    let stepping = eax & 0xF;
    let model = (eax >> 4) & 0xF;
    let family = (eax >> 8) & 0xF;
    let extended_model = (eax >> 16) & 0xF;
    let extended_family = (eax >> 20) & 0xFF;

    // Calculate display values
    let display_family = if family == 0xF {
        family + extended_family
    } else {
        family
    };

    let display_model = if family == 0x6 || family == 0xF {
        (extended_model << 4) + model
    } else {
        model
    };

    tracing::info!(
        "CPU: Family {:X}, Model {:X}, Stepping {:X}",
        display_family,
        display_model,
        stepping
    );

    // Intel architectures are Family 6
    if display_family != 0x6 {
        tracing::warn!("Non-Intel or very old Intel CPU detected");
        return Ok(CpuArchitecture::Unknown);
    }

    // Detect based on model number
    // Reference: Intel® 64 and IA-32 Architectures Software Developer's Manual
    let arch = match display_model {
        // Haswell (4th gen)
        0x3C | 0x45 | 0x46 => CpuArchitecture::Haswell,

        // Broadwell (5th gen)
        0x3D | 0x47 | 0x4F | 0x56 => CpuArchitecture::Broadwell,

        // Skylake (6th gen)
        0x4E | 0x5E => CpuArchitecture::Skylake,

        // Cascade Lake / Skylake-SP (server)
        0x55 => {
            // Differentiate by stepping
            if stepping >= 5 {
                CpuArchitecture::CascadeLake
            } else {
                CpuArchitecture::Skylake
            }
        }

        // Ice Lake
        0x7D | 0x7E | 0x6A | 0x6C => CpuArchitecture::IceLake,

        _ => {
            tracing::warn!("Unknown Intel CPU model: {:X}", display_model);
            // Default to Skylake for newer CPUs (most compatible)
            if display_model >= 0x4E {
                tracing::info!("Defaulting to Skylake architecture for compatibility");
                CpuArchitecture::Skylake
            } else {
                CpuArchitecture::Unknown
            }
        }
    };

    tracing::info!("Detected CPU architecture: {}", arch.name());

    Ok(arch)
}

// Architecture-specific event configurations
impl CpuArchitecture {
    /// Get L2 eviction event codes (architecture-specific)
    pub fn l2_eviction_events(&self) -> Vec<(u8, u8, &'static str)> {
        match self {
            CpuArchitecture::Skylake | CpuArchitecture::CascadeLake | CpuArchitecture::IceLake => {
                vec![(0xF2, 0x01, "L2OutSilent"), (0xF2, 0x02, "L2OutNonSilent")]
            }
            CpuArchitecture::Haswell | CpuArchitecture::Broadwell => {
                vec![(0xF2, 0x05, "L2OutClean"), (0xF2, 0x06, "L2OutDirty")]
            }
            CpuArchitecture::Unknown => {
                // Default to Skylake events
                vec![(0xF2, 0x01, "L2OutSilent"), (0xF2, 0x02, "L2OutNonSilent")]
            }
        }
    }

    /// Get L2 prefetch event codes (architecture-specific)
    pub fn l2_prefetch_events(&self) -> Vec<(u8, u8, &'static str)> {
        match self {
            CpuArchitecture::Skylake | CpuArchitecture::CascadeLake | CpuArchitecture::IceLake => {
                vec![
                    (0x24, 0x38, "L2PrefetchMiss"),
                    (0x24, 0xD8, "L2PrefetchHit"),
                ]
            }
            CpuArchitecture::Haswell | CpuArchitecture::Broadwell => {
                vec![
                    (0x24, 0x30, "L2PrefetchMiss"),
                    (0x24, 0x50, "L2PrefetchHit"),
                ]
            }
            CpuArchitecture::Unknown => {
                vec![
                    (0x24, 0x38, "L2PrefetchMiss"),
                    (0x24, 0xD8, "L2PrefetchHit"),
                ]
            }
        }
    }

    /// Check if architecture supports specific features
    pub fn supports_offcore_response(&self) -> bool {
        matches!(
            self,
            CpuArchitecture::Haswell
                | CpuArchitecture::Broadwell
                | CpuArchitecture::Skylake
                | CpuArchitecture::CascadeLake
                | CpuArchitecture::IceLake
        )
    }

    /// Get number of CHA (uncore) boxes
    pub fn cha_count(&self) -> Option<u32> {
        match self {
            CpuArchitecture::Skylake => Some(14),
            CpuArchitecture::CascadeLake => Some(26),
            CpuArchitecture::Haswell => Some(18),
            CpuArchitecture::Broadwell => Some(14),
            CpuArchitecture::IceLake => Some(24),
            CpuArchitecture::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_detection() {
        // This will run on the actual CPU
        let arch = *CPU_ARCH;
        println!("Detected architecture: {arch:?}");
        assert_ne!(arch, CpuArchitecture::Unknown);
    }

    #[test]
    fn test_architecture_features() {
        let skylake = CpuArchitecture::Skylake;
        assert!(skylake.supports_offcore_response());
        assert_eq!(skylake.cha_count(), Some(14));

        let events = skylake.l2_eviction_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].2, "L2OutSilent");
    }
}
