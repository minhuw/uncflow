// IIO (Integrated IO) metrics

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IioMetric {
    IIOTLBMiss,
    IIOTLBFull,
    IIOL1Miss,
    IIOL2Miss,
    IIOL3Miss,
    IIOContextMiss,
    IIOTLBHit,
    IIOTLB1Miss,
    IIOOccupancy,
    IIOFrequency,
    // PCIe bandwidth metrics (per channel and port)
    PCIeInBandwidth(usize, usize),  // (channel, port)
    PCIeOutBandwidth(usize, usize), // (channel, port)
}

impl IioMetric {
    pub fn name(&self) -> String {
        match self {
            IioMetric::IIOTLBMiss => "IIOTLBMiss".to_string(),
            IioMetric::IIOTLBFull => "IIOTLBFull".to_string(),
            IioMetric::IIOL1Miss => "IIOL1Miss".to_string(),
            IioMetric::IIOL2Miss => "IIOL2Miss".to_string(),
            IioMetric::IIOL3Miss => "IIOL3Miss".to_string(),
            IioMetric::IIOContextMiss => "IIOContextMiss".to_string(),
            IioMetric::IIOTLBHit => "IIOTLBHit".to_string(),
            IioMetric::IIOTLB1Miss => "IIOTLB1Miss".to_string(),
            IioMetric::IIOOccupancy => "IIOOccupancy".to_string(),
            IioMetric::IIOFrequency => "IIOFrequency".to_string(),
            IioMetric::PCIeInBandwidth(ch, port) => {
                format!("PCIe{ch}{port}InBandwidth")
            }
            IioMetric::PCIeOutBandwidth(ch, port) => {
                format!("PCIe{ch}{port}OutBandwidth")
            }
        }
    }

    pub fn all() -> Vec<IioMetric> {
        let mut metrics = vec![
            IioMetric::IIOTLBMiss,
            IioMetric::IIOTLBFull,
            IioMetric::IIOL1Miss,
            IioMetric::IIOL2Miss,
            IioMetric::IIOL3Miss,
            IioMetric::IIOContextMiss,
            IioMetric::IIOTLBHit,
            IioMetric::IIOTLB1Miss,
            IioMetric::IIOOccupancy,
            IioMetric::IIOFrequency,
        ];

        // Add PCIe bandwidth metrics for 3 channels and 4 ports each
        for ch in 0..3 {
            for port in 0..4 {
                metrics.push(IioMetric::PCIeInBandwidth(ch, port));
                metrics.push(IioMetric::PCIeOutBandwidth(ch, port));
            }
        }

        metrics
    }
}
