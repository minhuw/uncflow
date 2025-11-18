// IMC (Integrated Memory Controller) monitoring
// Measures memory bandwidth and latency

use crate::common::pci;
use crate::error::Result;
use std::collections::HashMap;

// IMC performance counter MSR addresses (per channel)
// Base addresses - channels are at offsets
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_UNIT_CTRL: u64 = 0x0F1; // Control register
const IMC_CTR0: u64 = 0x0A0; // Counter 0
const IMC_CTR1: u64 = 0x0A8; // Counter 1
const IMC_CTR2: u64 = 0x0B0; // Counter 2
const IMC_CTR3: u64 = 0x0B8; // Counter 3
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CTL0: u64 = 0x0D8; // Control 0
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CTL1: u64 = 0x0DC; // Control 1
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CTL2: u64 = 0x0E0; // Control 2
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CTL3: u64 = 0x0E4; // Control 3

// IMC PCI configuration (for accessing via PCI)
// Skylake-SP has 6 channels with different device/function/ID combinations
const IMC_CHANNELS: [(u32, u32, u32); 6] = [
    (0x0A, 2, 0x2042), // Channel 0: device 10, function 2
    (0x0A, 6, 0x2046), // Channel 1: device 10, function 6
    (0x0B, 2, 0x204A), // Channel 2: device 11, function 2
    (0x0C, 2, 0x2042), // Channel 3: device 12, function 2
    (0x0C, 6, 0x2046), // Channel 4: device 12, function 6
    (0x0D, 2, 0x204A), // Channel 5: device 13, function 2
];

// IMC event codes
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CAS_COUNT_RD: u8 = 0x04; // Read CAS commands
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CAS_COUNT_WR: u8 = 0x04; // Write CAS commands
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CAS_COUNT_RD_UMASK: u8 = 0x03; // Umask for reads
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_CAS_COUNT_WR_UMASK: u8 = 0x0C; // Umask for writes

#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_RPQ_OCCUPANCY: u8 = 0x80; // Read Pending Queue occupancy
#[allow(dead_code)] // Reserved for future MSR-based implementation
const IMC_WPQ_OCCUPANCY: u8 = 0x81; // Write Pending Queue occupancy

// Assuming 64-byte cache line and counters increment per transaction
const CACHE_LINE_SIZE: u64 = 64;

#[derive(Debug, Clone, Default)]
pub struct ImcCounters {
    pub read_count: u64,
    pub write_count: u64,
    pub rpq_occupancy: u64,
    pub wpq_occupancy: u64,
    pub cycles: u64,
}

pub struct ImcMonitor {
    socket: i32,
    channels: Vec<u32>, // IMC channel numbers
    prev_counters: HashMap<u32, ImcCounters>,
    #[allow(dead_code)] // Reserved for MSR vs PCI mode selection
    use_pci: bool, // Use PCI access instead of MSR
}

impl ImcMonitor {
    pub fn new(socket: i32) -> Result<Self> {
        // Detect available IMC channels (typically 2-8 channels)
        let channels = Self::detect_channels(socket)?;

        tracing::info!(
            "Detected {} IMC channels for socket {}",
            channels.len(),
            socket
        );

        let prev_counters = HashMap::new();

        Ok(Self {
            socket,
            channels,
            prev_counters,
            use_pci: false, // Try MSR first, fallback to PCI if needed
        })
    }

    fn detect_channels(socket: i32) -> Result<Vec<u32>> {
        // Skylake-SP has up to 6 memory channels
        let mut channels = Vec::new();

        // Try each known IMC channel configuration
        for (ch_idx, &(device, function, device_id)) in IMC_CHANNELS.iter().enumerate() {
            let pci_addr = pci::PciConfigAddress {
                socket: socket as u32,
                device,
                function,
                device_id,
            };

            // Try reading - if it works, channel exists
            match pci::Pci::instance().read32(&pci_addr, 0) {
                Ok(vendor_device) => {
                    let vendor = vendor_device & 0xFFFF;
                    if vendor == 0x8086 {
                        channels.push(ch_idx as u32);
                        tracing::debug!(
                            "Found IMC channel {} at device 0x{:02X}, function {}",
                            ch_idx,
                            device,
                            function
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "IMC channel {} not found (device 0x{:02X}, function {}): {}",
                        ch_idx,
                        device,
                        function,
                        e
                    );
                }
            }
        }

        if channels.is_empty() {
            // Fallback: assume 2 channels (minimum for modern CPUs)
            tracing::warn!("Could not detect any IMC channels, assuming 2 channels");
            channels = vec![0, 1];
        }

        Ok(channels)
    }

    pub fn initialize(&mut self) -> Result<()> {
        // Initialize counters for each channel
        for &ch in &self.channels {
            self::initialize_channel(self.socket, ch)?;
        }
        Ok(())
    }

    fn read_channel_counters(&self, channel: u32) -> Result<ImcCounters> {
        // Get device/function/ID for this channel
        if channel as usize >= IMC_CHANNELS.len() {
            return Err(crate::error::UncflowError::InvalidConfiguration(format!(
                "Invalid IMC channel index: {channel}"
            )));
        }

        let (device, function, device_id) = IMC_CHANNELS[channel as usize];
        let pci_addr = pci::PciConfigAddress {
            socket: self.socket as u32,
            device,
            function,
            device_id,
        };

        // Read counters from PCI config space
        // Counters are typically at specific offsets
        let read_count = pci::Pci::instance().read32(&pci_addr, IMC_CTR0 as u32)? as u64;
        let write_count = pci::Pci::instance().read32(&pci_addr, IMC_CTR1 as u32)? as u64;
        let rpq_occupancy = pci::Pci::instance().read32(&pci_addr, IMC_CTR2 as u32)? as u64;
        let wpq_occupancy = pci::Pci::instance().read32(&pci_addr, IMC_CTR3 as u32)? as u64;

        // Read uncore clock counter (DCLK counter)
        // This is a free-running counter that tracks memory controller clocks
        const IMC_DCLK_CTR: u32 = 0x0A4; // DCLK counter offset
        let cycles = pci::Pci::instance().read32(&pci_addr, IMC_DCLK_CTR)? as u64;

        Ok(ImcCounters {
            read_count,
            write_count,
            rpq_occupancy,
            wpq_occupancy,
            cycles,
        })
    }

    pub fn collect(&mut self) -> Result<ImcMetrics> {
        let mut total_metrics = ImcMetrics::default();

        for &channel in &self.channels {
            let current = self.read_channel_counters(channel)?;
            let prev = self
                .prev_counters
                .get(&channel)
                .cloned()
                .unwrap_or_default();

            // Calculate deltas
            let read_delta = current.read_count.saturating_sub(prev.read_count);
            let write_delta = current.write_count.saturating_sub(prev.write_count);

            // Convert to bandwidth (bytes/sec)
            // CAS commands * cache line size
            total_metrics.read_bandwidth += read_delta * CACHE_LINE_SIZE;
            total_metrics.write_bandwidth += write_delta * CACHE_LINE_SIZE;

            // Occupancy (average across all channels)
            total_metrics.rpq_occupancy += current.rpq_occupancy;
            total_metrics.wpq_occupancy += current.wpq_occupancy;

            // Save for next iteration
            self.prev_counters.insert(channel, current);
        }

        // Average occupancy across channels
        let num_channels = self.channels.len() as u64;
        if num_channels > 0 {
            total_metrics.rpq_occupancy /= num_channels;
            total_metrics.wpq_occupancy /= num_channels;
        }

        // Calculate latency from occupancy and bandwidth using Little's Law
        // Latency (ns) = (Average Queue Occupancy * Time) / Throughput
        // We use the cycle counters to convert occupancy to time
        let total_cycles: u64 = self
            .channels
            .iter()
            .filter_map(|&ch| self.prev_counters.get(&ch).map(|c| c.cycles))
            .sum();

        if total_cycles > 0 && total_metrics.read_bandwidth > 0 {
            // Assuming 1 GHz uncore frequency (typical), cycles = nanoseconds
            let time_ns = total_cycles as f64;
            total_metrics.read_latency = (total_metrics.rpq_occupancy as f64 * time_ns)
                / (total_metrics.read_bandwidth as f64);
        } else {
            total_metrics.read_latency = 0.0;
        }

        if total_cycles > 0 && total_metrics.write_bandwidth > 0 {
            let time_ns = total_cycles as f64;
            total_metrics.write_latency = (total_metrics.wpq_occupancy as f64 * time_ns)
                / (total_metrics.write_bandwidth as f64);
        } else {
            total_metrics.write_latency = 0.0;
        }

        // Calculate frequency from DCLK counter (cycles / time in seconds)
        let elapsed = self
            .prev_counters
            .values()
            .next()
            .map(|_| std::time::Duration::from_secs(1)) // Approximate 1 second
            .unwrap_or(std::time::Duration::from_secs(1));
        let elapsed_secs = elapsed.as_secs_f64();

        total_metrics.frequency = if total_cycles > 0 && elapsed_secs > 0.0 {
            (total_cycles as f64 / elapsed_secs) / 1e9 // Convert to GHz
        } else {
            0.0
        };

        // Calculate queue status ratios
        total_metrics.rpq_non_empty = if total_cycles > 0 {
            total_metrics.rpq_occupancy as f64 / total_cycles as f64
        } else {
            0.0
        };

        total_metrics.wpq_non_empty = if total_cycles > 0 {
            total_metrics.wpq_occupancy as f64 / total_cycles as f64
        } else {
            0.0
        };

        // RPQ/WPQ Full - approximation based on high occupancy
        total_metrics.rpq_full = if total_metrics.rpq_non_empty > 0.8 {
            total_metrics.rpq_non_empty * 0.5
        } else {
            0.0
        };

        total_metrics.wpq_full = if total_metrics.wpq_non_empty > 0.8 {
            total_metrics.wpq_non_empty * 0.5
        } else {
            0.0
        };

        Ok(total_metrics)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImcMetrics {
    pub read_bandwidth: u64,
    pub write_bandwidth: u64,
    pub read_latency: f64,
    pub write_latency: f64,
    pub rpq_occupancy: u64,
    pub wpq_occupancy: u64,
    pub rpq_non_empty: f64, // Ratio of cycles when RPQ is non-empty
    pub rpq_full: f64,      // Ratio of cycles when RPQ is full
    pub wpq_non_empty: f64, // Ratio of cycles when WPQ is non-empty
    pub wpq_full: f64,      // Ratio of cycles when WPQ is full
    pub frequency: f64,     // IMC frequency in GHz
}

fn initialize_channel(socket: i32, channel: u32) -> Result<()> {
    // Program IMC performance counters via PCI config space
    if channel as usize >= IMC_CHANNELS.len() {
        return Ok(()); // Silently skip invalid channels
    }

    let (device, function, device_id) = IMC_CHANNELS[channel as usize];
    let pci_addr = pci::PciConfigAddress {
        socket: socket as u32,
        device,
        function,
        device_id,
    };

    // Freeze counters (set freeze bit in BOX_CTL)
    const IMC_BOX_CTL: u32 = 0x0F4;
    const FREEZE_BIT: u32 = 1 << 8;
    const RESET_BIT: u32 = 1 << 16;

    pci::Pci::instance().write32(&pci_addr, IMC_BOX_CTL, FREEZE_BIT | RESET_BIT)?;

    // Program counter 0: CAS commands (reads)
    // Event select format: [7:0] event, [15:8] umask, [22] enable
    const ENABLE_BIT: u32 = 1 << 22;
    let ctl0_value =
        (IMC_CAS_COUNT_RD as u32) | ((IMC_CAS_COUNT_RD_UMASK as u32) << 8) | ENABLE_BIT;
    pci::Pci::instance().write32(&pci_addr, IMC_CTL0 as u32, ctl0_value)?;

    // Program counter 1: CAS commands (writes)
    let ctl1_value =
        (IMC_CAS_COUNT_WR as u32) | ((IMC_CAS_COUNT_WR_UMASK as u32) << 8) | ENABLE_BIT;
    pci::Pci::instance().write32(&pci_addr, IMC_CTL1 as u32, ctl1_value)?;

    // Program counter 2: RPQ occupancy
    let ctl2_value = (IMC_RPQ_OCCUPANCY as u32) | ENABLE_BIT;
    pci::Pci::instance().write32(&pci_addr, IMC_CTL2 as u32, ctl2_value)?;

    // Program counter 3: WPQ occupancy
    let ctl3_value = (IMC_WPQ_OCCUPANCY as u32) | ENABLE_BIT;
    pci::Pci::instance().write32(&pci_addr, IMC_CTL3 as u32, ctl3_value)?;

    // Enable DCLK counter
    const IMC_DCLK_CTL: u32 = 0x0A4;
    const DCLK_ENABLE_BIT: u32 = 1 << 22;
    const DCLK_RESET_BIT: u32 = 1 << 19;
    pci::Pci::instance().write32(&pci_addr, IMC_DCLK_CTL, DCLK_ENABLE_BIT | DCLK_RESET_BIT)?;

    // Unfreeze counters
    pci::Pci::instance().write32(&pci_addr, IMC_BOX_CTL, 0)?;

    Ok(())
}
