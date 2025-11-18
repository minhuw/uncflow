// Core PMU metrics for Skylake architecture
use crate::metric_enum;

metric_enum! {
    pub enum CoreMetric {
        IPC => "IPC",
        Instructions => "instructions",
        Cycles => "cycles",
        L3CacheMiss => "L3CacheMissNum",
        L3CacheRef => "L3CacheRef",
        L2CacheMiss => "L2CacheMissNum",
        L2CacheRef => "L2CacheRef",
        L3CacheHitRatio => "L3CacheHitRatio",
        L2CacheHitRatio => "L2CacheHitRatio",
        L2PrefetchMiss => "L2PrefetchMiss",
        L2PrefetchHit => "L2PrefetchHit",
        L2OutSilent => "L2OutSilent",
        L2OutNonSilent => "L2OutNonSilent",
        L2In => "L2In",
        L2Writeback => "L2Writeback",
        L3MPI => "L3MPI",
        L2MPI => "L2MPI",
        ElapsedTime => "elapsedTime",
    }
}
