use crate::metric_enum;

metric_enum! {
    pub enum RaplMetric {
        PackageEnergy => "PackageEnergy",
        CoreEnergy => "CoreEnergy",
        DramEnergy => "DRAMEnergy",
        PackagePower => "PackagePower",
        CorePower => "CorePower",
        DramPower => "DRAMPower",
    }
}
