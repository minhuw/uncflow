use crate::metric_enum;

metric_enum! {
    pub enum RdtMetric {
        LocalMemoryBandwidth => "LocalMemoryBandwidth",
        RemoteMemoryBandwidth => "RemoteMemoryBandwidth",
        TotalMemoryBandwidth => "TotalMemoryBandwidth",
        LlcOccupancy => "CMTLLCOccupancy",
    }
}
