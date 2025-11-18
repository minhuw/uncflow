// IMC (Integrated Memory Controller) metrics

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImcMetric {
    // Bandwidth metrics
    MemoryReadBandwidth,
    MemoryWriteBandwidth,
    MemoryLocalReadBandwidth,
    MemoryLocalWriteBandwidth,
    MemoryRemoteReadBandwidth,
    MemoryRemoteWriteBandwidth,

    // Latency metrics
    MemoryReadLatency,
    MemoryWriteLatency,

    // Queue occupancy metrics
    MemoryRPQOccupancy,
    MemoryWPQOccupancy,

    // Queue status metrics (new)
    IMCRPQNonEmpty,
    IMCRPQFull,
    IMCWPQNonEmpty,
    IMCWPQFull,

    // Frequency metric
    IMCFrequency,

    // NUMA locality ratios (new)
    MemoryLocalReadRatio,
    MemoryLocalWriteRatio,
}

impl ImcMetric {
    pub fn name(&self) -> &'static str {
        match self {
            ImcMetric::MemoryReadBandwidth => "MemoryReadBandwidth",
            ImcMetric::MemoryWriteBandwidth => "MemoryWriteBandwidth",
            ImcMetric::MemoryLocalReadBandwidth => "MemoryLocalReadBandwidth",
            ImcMetric::MemoryLocalWriteBandwidth => "MemoryLocalWriteBandwidth",
            ImcMetric::MemoryRemoteReadBandwidth => "MemoryRemoteReadBandwidth",
            ImcMetric::MemoryRemoteWriteBandwidth => "MemoryRemoteWriteBandwidth",
            ImcMetric::MemoryReadLatency => "IMCReadLatency",
            ImcMetric::MemoryWriteLatency => "IMCWriteLatency",
            ImcMetric::MemoryRPQOccupancy => "MemoryRPQOccupancy",
            ImcMetric::MemoryWPQOccupancy => "MemoryWPQOccupancy",
            ImcMetric::IMCRPQNonEmpty => "IMCRPQNonEmpty",
            ImcMetric::IMCRPQFull => "IMCRPQFull",
            ImcMetric::IMCWPQNonEmpty => "IMCWPQNonEmpty",
            ImcMetric::IMCWPQFull => "IMCWPQFull",
            ImcMetric::IMCFrequency => "IMCFrequency",
            ImcMetric::MemoryLocalReadRatio => "MemoryLocalReadRatio",
            ImcMetric::MemoryLocalWriteRatio => "MemoryLocalWriteRatio",
        }
    }

    pub fn all() -> Vec<ImcMetric> {
        vec![
            // Bandwidth
            ImcMetric::MemoryReadBandwidth,
            ImcMetric::MemoryWriteBandwidth,
            ImcMetric::MemoryLocalReadBandwidth,
            ImcMetric::MemoryLocalWriteBandwidth,
            ImcMetric::MemoryRemoteReadBandwidth,
            ImcMetric::MemoryRemoteWriteBandwidth,
            // Latency
            ImcMetric::MemoryReadLatency,
            ImcMetric::MemoryWriteLatency,
            // Queue occupancy
            ImcMetric::MemoryRPQOccupancy,
            ImcMetric::MemoryWPQOccupancy,
            // Queue status
            ImcMetric::IMCRPQNonEmpty,
            ImcMetric::IMCRPQFull,
            ImcMetric::IMCWPQNonEmpty,
            ImcMetric::IMCWPQFull,
            // Frequency
            ImcMetric::IMCFrequency,
            // NUMA ratios
            ImcMetric::MemoryLocalReadRatio,
            ImcMetric::MemoryLocalWriteRatio,
        ]
    }
}
