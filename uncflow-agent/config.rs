use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExportConfig {
    pub sockets: Vec<i32>,
    pub cores: Vec<i32>,
    pub core_labels: HashMap<i32, String>,
}

impl ExportConfig {
    /// Create a new configuration from sockets and cores
    pub fn new(sockets: Vec<i32>, cores: Vec<i32>) -> Self {
        // Auto-generate labels for cores
        let core_labels: HashMap<i32, String> = cores
            .iter()
            .map(|&core| (core, format!("core_{core}")))
            .collect();

        Self {
            sockets,
            cores,
            core_labels,
        }
    }

    /// Auto-detect all available CPUs in the system
    pub fn auto_detect() -> Self {
        let cores = Self::detect_online_cpus();
        let sockets = Self::detect_sockets(&cores);

        tracing::info!(
            "Auto-detected {} sockets, {} cores",
            sockets.len(),
            cores.len()
        );

        Self::new(sockets, cores)
    }

    /// Detect online CPUs from /sys/devices/system/cpu/online
    pub fn detect_online_cpus() -> Vec<i32> {
        std::fs::read_to_string("/sys/devices/system/cpu/online")
            .ok()
            .and_then(|s| Self::parse_cpu_list(&s))
            .unwrap_or_else(|| {
                tracing::warn!("Failed to detect online CPUs, using default: 0-7");
                (0..8).collect()
            })
    }

    /// Parse CPU list like "0-3,8-11" into Vec<i32>
    fn parse_cpu_list(s: &str) -> Option<Vec<i32>> {
        let mut cpus = Vec::new();
        for part in s.trim().split(',') {
            if let Some((start, end)) = part.split_once('-') {
                let start: i32 = start.parse().ok()?;
                let end: i32 = end.parse().ok()?;
                cpus.extend(start..=end);
            } else {
                cpus.push(part.parse().ok()?);
            }
        }
        Some(cpus)
    }

    /// Detect which sockets the cores belong to
    pub fn detect_sockets(cores: &[i32]) -> Vec<i32> {
        let mut sockets = std::collections::HashSet::new();

        for &core in cores {
            let socket_path =
                format!("/sys/devices/system/cpu/cpu{core}/topology/physical_package_id");
            if let Ok(socket_str) = std::fs::read_to_string(&socket_path) {
                if let Ok(socket) = socket_str.trim().parse::<i32>() {
                    sockets.insert(socket);
                }
            }
        }

        let mut socket_vec: Vec<i32> = sockets.into_iter().collect();
        socket_vec.sort_unstable();

        if socket_vec.is_empty() {
            socket_vec.push(0); // Default to socket 0
        }

        socket_vec
    }
}
