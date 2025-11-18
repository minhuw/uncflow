// IRP (IO Request Processing) metrics
use crate::metric_enum;

metric_enum! {
    pub enum IrpMetric {
        IRPLatency => "IRPLatency",
        IRPAnyOccupancy => "IRPAnyOccupancy",
        IRPPCIeReadBandwidth => "IRPPCIeReadBandwidth",
        IRPRFOBandwidth => "IRPRFOBandwidth",
        IRPAllBandwidth => "IRPAllBandwidth",
        IRPPCIItoMBandwidth => "IRPPCIItoMBandwidth",
        IRPWbMtoIBandwidth => "IRPWbMtoIBandwidth",
        IRPCLFlushBandwidth => "IRPCLFlushBandwidth",
        IRPFrequency => "IRPFrequency",
    }
}
