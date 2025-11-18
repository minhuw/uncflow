//! Declarative macros to reduce boilerplate across the uncflow codebase

/// Define a metric enum with automatic `name()` and `all()` implementations
///
/// # Example
/// ```
/// use uncflow::metric_enum;
///
/// metric_enum! {
///     pub enum RaplMetric {
///         PackageEnergy => "PackageEnergy",
///         CoreEnergy => "CoreEnergy",
///         DramEnergy => "DRAMEnergy",
///     }
/// }
///
/// // Usage
/// let metric = RaplMetric::PackageEnergy;
/// assert_eq!(metric.name(), "PackageEnergy");
/// assert_eq!(RaplMetric::all().len(), 3);
/// ```
///
/// Expands to:
/// - An enum with Debug, Clone, Copy, PartialEq, Eq, Hash derives
/// - A `name(&self) -> &'static str` method
/// - An `all() -> Vec<Self>` method
#[macro_export]
macro_rules! metric_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident => $str:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant,)*
        }

        impl $name {
            pub fn name(&self) -> &'static str {
                match self {
                    $($name::$variant => $str,)*
                }
            }

            pub fn all() -> Vec<$name> {
                vec![$($name::$variant,)*]
            }
        }
    };
}

/// Initialize an exporter with standard error handling
///
/// # Example
/// ```ignore
/// // In orchestrator::collector::Collector::new()
/// init_exporter!(
///     collector,
///     collector_config,
///     config,
///     rapl_exporter,
///     rapl,
///     RaplMetricExporter,
///     "RAPL"
/// );
/// ```
#[macro_export]
macro_rules! init_exporter {
    (
        $collector:expr,
        $collector_config:expr,
        $config:expr,
        $field:ident,
        $flag:ident,
        $Exporter:ty,
        $name:literal
    ) => {
        if $collector_config.$flag {
            match <$Exporter>::new($config.clone()) {
                Ok(exporter) => {
                    $collector.$field = Some(std::sync::Arc::new(exporter));
                    tracing::info!(concat!($name, " exporter initialized"));
                }
                Err(e) => {
                    tracing::error!(concat!("Failed to initialize ", $name, " exporter: {}"), e);
                }
            }
        }
    };
}

/// Spawn a collector task for an exporter if it exists
///
/// # Example
/// ```ignore
/// // In orchestrator::collector::Collector::collection_loop()
/// let mut tasks = Vec::new();
/// spawn_collector!(tasks, &self.rapl_exporter);
/// ```
#[macro_export]
macro_rules! spawn_collector {
    ($tasks:expr, $exporter:expr) => {
        if let Some(exporter) = $exporter {
            let exp = std::sync::Arc::clone(exporter);
            $tasks.push(tokio::spawn(async move {
                exp.collect().await;
            }));
        }
    };
}

/// Gather metrics from an exporter's registry
///
/// # Example
/// ```ignore
/// // In main.rs metrics handler
/// let mut buffer = Vec::new();
/// gather_metrics!(buffer, encoder, state.rapl_exporter, "RAPL");
/// ```
#[macro_export]
macro_rules! gather_metrics {
    ($buffer:expr, $encoder:expr, $exporter:expr, $name:literal) => {
        if let Some(ref exporter) = $exporter {
            let metric_families = exporter.registry().gather();
            if let Err(e) = $encoder.encode(&metric_families, &mut $buffer) {
                tracing::error!(concat!("Failed to encode ", $name, " metrics: {}"), e);
            }
        }
    };
}

/// Define an enum with name() and all() methods, plus custom data per variant
///
/// # Example
/// ```
/// use uncflow::enum_with_data;
///
/// enum_with_data! {
///     pub enum LLCState: u32 {
///         M => ("M", 0x40),
///         E => ("E", 0x20),
///         S => ("S", 0x02),
///         I => ("I", 0x01),
///     }
///     impl value -> u32
/// }
///
/// let state = LLCState::M;
/// assert_eq!(state.name(), "M");
/// assert_eq!(state.value(), 0x40);
/// ```
#[macro_export]
macro_rules! enum_with_data {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $data_type:ty {
            $($variant:ident => ($str:literal, $data:expr)),* $(,)?
        }
        impl $method:ident -> $return_type:ty
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant,)*
        }

        impl $name {
            pub fn name(&self) -> &'static str {
                match self {
                    $($name::$variant => $str,)*
                }
            }

            pub fn $method(&self) -> $return_type {
                match self {
                    $($name::$variant => $data,)*
                }
            }

            pub fn all() -> Vec<$name> {
                vec![$($name::$variant,)*]
            }
        }
    };
}

/// Define an enum with opcodes (returns tuple)
///
/// # Example
/// ```ignore
/// enum_with_opcodes! {
///     pub enum TransactionType {
///         PCIeRead => ("PCIeRead", 0x21E, 0),
///         RFO => ("RFO", 0x200, 0),
///     }
/// }
/// ```
#[macro_export]
macro_rules! enum_with_opcodes {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident => ($str:literal, $opc0:expr, $opc1:expr)),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant,)*
        }

        impl $name {
            pub fn name(&self) -> &'static str {
                match self {
                    $($name::$variant => $str,)*
                }
            }

            pub fn opcodes(&self) -> (u32, u32) {
                match self {
                    $($name::$variant => ($opc0, $opc1),)*
                }
            }

            pub fn all() -> Vec<$name> {
                vec![$($name::$variant,)*]
            }
        }
    };
}
