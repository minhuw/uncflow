//! RAPL (Running Average Power Limit) register definitions for Skylake-SP
//!
//! RAPL provides energy consumption monitoring for various power domains.
//!
//! ## References
//!
//! - Intel® 64 and IA-32 Architectures Software Developer's Manual, Volume 3B
//! - Section 14.9: Platform Specific Power Management Support

use crate::register::RegisterLayout;

/// MSR addresses for RAPL
pub mod msr {
    /// RAPL Power Unit MSR - Defines energy, power, and time units
    pub const MSR_RAPL_POWER_UNIT: u64 = 0x606;

    /// Package Energy Status - Total package energy consumption
    pub const MSR_PKG_ENERGY_STATUS: u64 = 0x611;

    /// PP0 Energy Status - Core energy consumption
    pub const MSR_PP0_ENERGY_STATUS: u64 = 0x639;

    /// DRAM Energy Status - Memory controller energy consumption
    pub const MSR_DRAM_ENERGY_STATUS: u64 = 0x619;

    /// Package Power Limit - Configure package power limits
    pub const MSR_PKG_POWER_LIMIT: u64 = 0x610;

    /// Package Power Info - Package TDP and limits
    pub const MSR_PKG_POWER_INFO: u64 = 0x614;

    /// PP0 Power Limit - Core power limits
    pub const MSR_PP0_POWER_LIMIT: u64 = 0x638;

    /// DRAM Power Limit - Memory power limits
    pub const MSR_DRAM_POWER_LIMIT: u64 = 0x618;
}

/// RAPL Power Unit Register layout
///
/// Defines the units for energy, power, and time measurements.
///
/// ## Register Format
///
/// | Bits   | Field        | Description                           |
/// |--------|--------------|---------------------------------------|
/// | 0-3    | power_units  | Power units (1/2^ESU watts)          |
/// | 4-7    | reserved     |                                       |
/// | 8-12   | energy_units | Energy units (1/2^ESU joules)        |
/// | 13-15  | reserved     |                                       |
/// | 16-19  | time_units   | Time units (1/2^TU seconds)          |
/// | 20-63  | reserved     |                                       |
#[derive(Debug, Clone, Copy, Default)]
pub struct RaplPowerUnit {
    /// Power units: watts = value * (1.0 / 2^power_units)
    pub power_units: u8,

    /// Energy units: joules = value * (1.0 / 2^energy_units)
    pub energy_units: u8,

    /// Time units: seconds = value * (1.0 / 2^time_units)
    pub time_units: u8,
}

impl RegisterLayout for RaplPowerUnit {
    fn to_msr_value(&self) -> u64 {
        (self.power_units as u64 & 0x0F)
            | ((self.energy_units as u64 & 0x1F) << 8)
            | ((self.time_units as u64 & 0x0F) << 16)
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            power_units: (value & 0x0F) as u8,
            energy_units: ((value >> 8) & 0x1F) as u8,
            time_units: ((value >> 16) & 0x0F) as u8,
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.power_units > 15 {
            return Err("Power units must be <= 15 (4 bits)");
        }
        if self.energy_units > 31 {
            return Err("Energy units must be <= 31 (5 bits)");
        }
        if self.time_units > 15 {
            return Err("Time units must be <= 15 (4 bits)");
        }
        Ok(())
    }
}

impl RaplPowerUnit {
    /// Get power unit multiplier (watts per LSB)
    pub fn power_unit_multiplier(&self) -> f64 {
        1.0 / (1u64 << self.power_units) as f64
    }

    /// Get energy unit multiplier (joules per LSB)
    pub fn energy_unit_multiplier(&self) -> f64 {
        1.0 / (1u64 << self.energy_units) as f64
    }

    /// Get time unit multiplier (seconds per LSB)
    pub fn time_unit_multiplier(&self) -> f64 {
        1.0 / (1u64 << self.time_units) as f64
    }
}

/// RAPL Power Limit Register layout
///
/// Configures power limits and time windows for a power domain.
///
/// ## Register Format
///
/// | Bits   | Field          | Description                        |
/// |--------|----------------|------------------------------------|
/// | 0-14   | power_limit_1  | Power limit 1 (watts)             |
/// | 15     | enable_1       | Enable power limit 1              |
/// | 16     | clamp_1        | Clamp to power limit 1            |
/// | 17-23  | time_window_1  | Time window 1                     |
/// | 24-31  | reserved       |                                    |
/// | 32-46  | power_limit_2  | Power limit 2 (watts)             |
/// | 47     | enable_2       | Enable power limit 2              |
/// | 48     | clamp_2        | Clamp to power limit 2            |
/// | 49-55  | time_window_2  | Time window 2                     |
/// | 56-62  | reserved       |                                    |
/// | 63     | lock           | Lock register                     |
#[derive(Debug, Clone, Copy, Default)]
pub struct RaplPowerLimit {
    /// Power limit 1 (in watts, scaled by power units)
    pub power_limit_1: u16,

    /// Enable power limit 1
    pub enable_1: bool,

    /// Clamp to power limit 1
    pub clamp_1: bool,

    /// Time window 1 (bits 17-23)
    pub time_window_1: u8,

    /// Power limit 2 (in watts, scaled by power units)
    pub power_limit_2: u16,

    /// Enable power limit 2
    pub enable_2: bool,

    /// Clamp to power limit 2
    pub clamp_2: bool,

    /// Time window 2 (bits 49-55)
    pub time_window_2: u8,

    /// Lock register (prevents further writes until reset)
    pub lock: bool,
}

impl RegisterLayout for RaplPowerLimit {
    fn to_msr_value(&self) -> u64 {
        (self.power_limit_1 as u64 & 0x7FFF)
            | (if self.enable_1 { 1 << 15 } else { 0 })
            | (if self.clamp_1 { 1 << 16 } else { 0 })
            | ((self.time_window_1 as u64 & 0x7F) << 17)
            | ((self.power_limit_2 as u64 & 0x7FFF) << 32)
            | (if self.enable_2 { 1 << 47 } else { 0 })
            | (if self.clamp_2 { 1 << 48 } else { 0 })
            | ((self.time_window_2 as u64 & 0x7F) << 49)
            | (if self.lock { 1 << 63 } else { 0 })
    }

    fn from_msr_value(value: u64) -> Self {
        Self {
            power_limit_1: (value & 0x7FFF) as u16,
            enable_1: (value & (1 << 15)) != 0,
            clamp_1: (value & (1 << 16)) != 0,
            time_window_1: ((value >> 17) & 0x7F) as u8,
            power_limit_2: ((value >> 32) & 0x7FFF) as u16,
            enable_2: (value & (1 << 47)) != 0,
            clamp_2: (value & (1 << 48)) != 0,
            time_window_2: ((value >> 49) & 0x7F) as u8,
            lock: (value & (1 << 63)) != 0,
        }
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.power_limit_1 > 0x7FFF {
            return Err("Power limit 1 must be <= 0x7FFF (15 bits)");
        }
        if self.time_window_1 > 127 {
            return Err("Time window 1 must be <= 127 (7 bits)");
        }
        if self.power_limit_2 > 0x7FFF {
            return Err("Power limit 2 must be <= 0x7FFF (15 bits)");
        }
        if self.time_window_2 > 127 {
            return Err("Time window 2 must be <= 127 (7 bits)");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rapl_power_unit_round_trip() {
        let unit = RaplPowerUnit {
            power_units: 3,
            energy_units: 14,
            time_units: 10,
        };

        let value = unit.to_msr_value();
        let decoded = RaplPowerUnit::from_msr_value(value);

        assert_eq!(decoded.power_units, unit.power_units);
        assert_eq!(decoded.energy_units, unit.energy_units);
        assert_eq!(decoded.time_units, unit.time_units);
    }

    #[test]
    fn test_rapl_power_unit_multipliers() {
        let unit = RaplPowerUnit {
            power_units: 3,
            energy_units: 14,
            time_units: 10,
        };

        assert_eq!(unit.power_unit_multiplier(), 1.0 / 8.0);
        assert_eq!(unit.energy_unit_multiplier(), 1.0 / 16384.0);
        assert_eq!(unit.time_unit_multiplier(), 1.0 / 1024.0);
    }

    #[test]
    fn test_rapl_power_limit_round_trip() {
        let limit = RaplPowerLimit {
            power_limit_1: 100,
            enable_1: true,
            clamp_1: true,
            time_window_1: 50,
            power_limit_2: 120,
            enable_2: true,
            clamp_2: false,
            time_window_2: 60,
            lock: false,
        };

        let value = limit.to_msr_value();
        let decoded = RaplPowerLimit::from_msr_value(value);

        assert_eq!(decoded.power_limit_1, limit.power_limit_1);
        assert_eq!(decoded.enable_1, limit.enable_1);
        assert_eq!(decoded.time_window_1, limit.time_window_1);
        assert_eq!(decoded.power_limit_2, limit.power_limit_2);
        assert_eq!(decoded.enable_2, limit.enable_2);
    }
}
