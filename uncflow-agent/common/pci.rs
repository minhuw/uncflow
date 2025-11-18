use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::{Result, UncflowError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciConfigAddress {
    pub socket: u32,
    pub device: u32,
    pub function: u32,
    pub device_id: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct PciAddress {
    pub group_number: u32,
    pub bus: u32,
    pub device: u32,
    pub function: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct McfgRecord {
    base_address: u64,
    pci_segment_group: u16,
    start_bus: u8,
    end_bus: u8,
    _reserved: [u8; 4],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct McfgHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
    _reserved: [u8; 8],
}

impl McfgHeader {
    fn n_records(&self) -> usize {
        let header_size = std::mem::size_of::<McfgHeader>();
        let record_size = std::mem::size_of::<McfgRecord>();
        ((self.length as usize) - header_size) / record_size
    }
}

pub struct PciHandle {
    file: parking_lot::Mutex<File>,
    #[allow(dead_code)] // Stored for validation/logging
    address: PciAddress,
}

impl PciHandle {
    pub fn new(address: PciAddress) -> Result<Self> {
        let path = Self::get_pci_path(address)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                UncflowError::PciError(format!(
                    "Failed to open PCI device {:04X}:{:02X}:{:02X}.{}: {}",
                    address.group_number, address.bus, address.device, address.function, e
                ))
            })?;

        Ok(Self {
            file: parking_lot::Mutex::new(file),
            address,
        })
    }

    fn get_pci_path(address: PciAddress) -> Result<PathBuf> {
        let base_path = if std::env::var("DOCKER_RUNNING").is_ok() {
            "/pcm/proc/bus/pci"
        } else {
            "/proc/bus/pci"
        };

        let path = if address.group_number > 0 {
            format!(
                "{}/{:04x}:{:02x}/{:02x}.{}",
                base_path, address.group_number, address.bus, address.device, address.function
            )
        } else {
            format!(
                "{}/{:02x}/{:02x}.{}",
                base_path, address.bus, address.device, address.function
            )
        };

        Ok(PathBuf::from(path))
    }

    pub fn read32(&self, offset: u32) -> Result<u32> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
            UncflowError::PciError(format!("Failed to seek to offset {offset}: {e}"))
        })?;

        let mut buffer = [0u8; 4];
        file.read_exact(&mut buffer).map_err(|e| {
            UncflowError::PciError(format!("Failed to read at offset {offset}: {e}"))
        })?;

        Ok(u32::from_le_bytes(buffer))
    }

    pub fn write32(&self, offset: u32, value: u32) -> Result<()> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
            UncflowError::PciError(format!("Failed to seek to offset {offset}: {e}"))
        })?;

        file.write_all(&value.to_le_bytes()).map_err(|e| {
            UncflowError::PciError(format!("Failed to write at offset {offset}: {e}"))
        })?;

        Ok(())
    }

    pub fn read64(&self, offset: u32) -> Result<u64> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
            UncflowError::PciError(format!("Failed to seek to offset {offset}: {e}"))
        })?;

        let mut buffer = [0u8; 8];
        file.read_exact(&mut buffer).map_err(|e| {
            UncflowError::PciError(format!("Failed to read at offset {offset}: {e}"))
        })?;

        Ok(u64::from_le_bytes(buffer))
    }
}

pub struct Mcfg {
    records: Vec<McfgRecord>,
    group_bus_map: RwLock<HashMap<PciConfigAddress, PciAddress>>,
}

impl Mcfg {
    fn new() -> Result<Self> {
        let mcfg_path = if std::env::var("DOCKER_RUNNING").is_ok() {
            "/pcm/sys/firmware/acpi/tables/MCFG"
        } else {
            "/sys/firmware/acpi/tables/MCFG"
        };

        let mut file = File::open(mcfg_path)
            .map_err(|e| UncflowError::PciError(format!("Failed to open MCFG table: {e}")))?;

        let mut header_bytes = vec![0u8; std::mem::size_of::<McfgHeader>()];
        file.read_exact(&mut header_bytes)
            .map_err(|e| UncflowError::PciError(format!("Failed to read MCFG header: {e}")))?;

        let header: McfgHeader = unsafe { std::ptr::read(header_bytes.as_ptr() as *const _) };
        let n_records = header.n_records();

        let mut records = Vec::with_capacity(n_records);
        for _ in 0..n_records {
            let mut record_bytes = vec![0u8; std::mem::size_of::<McfgRecord>()];
            file.read_exact(&mut record_bytes)
                .map_err(|e| UncflowError::PciError(format!("Failed to read MCFG record: {e}")))?;

            let record: McfgRecord = unsafe { std::ptr::read(record_bytes.as_ptr() as *const _) };
            records.push(record);
        }

        Ok(Self {
            records,
            group_bus_map: RwLock::new(HashMap::new()),
        })
    }

    pub fn instance() -> &'static Mcfg {
        static INSTANCE: Lazy<Result<Mcfg>> = Lazy::new(Mcfg::new);
        INSTANCE.as_ref().unwrap()
    }

    fn validate_pci_address(
        &self,
        group: u32,
        bus: u32,
        device: u32,
        function: u32,
        device_id: u32,
    ) -> bool {
        let address = PciAddress {
            group_number: group,
            bus,
            device,
            function,
        };

        if let Ok(handle) = PciHandle::new(address) {
            if let Ok(value) = handle.read32(0) {
                let vendor = value & 0xFFFF;
                let device = (value >> 16) & 0xFFFF;
                return vendor == 0x8086 && device == device_id;
            }
        }
        false
    }

    pub fn find_group_bus(&self, config_addr: &PciConfigAddress) -> Result<PciAddress> {
        {
            let map = self.group_bus_map.read();
            if let Some(&addr) = map.get(config_addr) {
                return Ok(addr);
            }
        }

        let mut candidates = Vec::new();

        for record in &self.records {
            let pci_segment = record.pci_segment_group;
            let start_bus = record.start_bus;
            let end_bus = record.end_bus;

            for bus in start_bus..=end_bus {
                if self.validate_pci_address(
                    pci_segment as u32,
                    bus as u32,
                    config_addr.device,
                    config_addr.function,
                    config_addr.device_id,
                ) {
                    tracing::warn!(
                        "Located PCI device {:04X}:{:02X}:{:02X}.{}",
                        pci_segment,
                        bus,
                        config_addr.device,
                        config_addr.function
                    );
                    candidates.push(PciAddress {
                        group_number: pci_segment as u32,
                        bus: bus as u32,
                        device: config_addr.device,
                        function: config_addr.function,
                    });
                }
            }
        }

        if (config_addr.socket as usize) < candidates.len() {
            let addr = candidates[config_addr.socket as usize];
            let mut map = self.group_bus_map.write();
            map.insert(*config_addr, addr);
            return Ok(addr);
        }

        Err(UncflowError::PciError(format!(
            "Cannot find PCI device for socket {} device {} function {}",
            config_addr.socket, config_addr.device, config_addr.function
        )))
    }
}

pub struct Pci {
    handles: RwLock<HashMap<PciConfigAddress, Arc<PciHandle>>>,
}

impl Pci {
    fn new() -> Self {
        Self {
            handles: RwLock::new(HashMap::new()),
        }
    }

    pub fn instance() -> &'static Pci {
        static INSTANCE: Lazy<Pci> = Lazy::new(Pci::new);
        &INSTANCE
    }

    fn get_or_create_handle(&self, config_addr: &PciConfigAddress) -> Result<Arc<PciHandle>> {
        {
            let handles = self.handles.read();
            if let Some(handle) = handles.get(config_addr) {
                return Ok(Arc::clone(handle));
            }
        }

        let address = Mcfg::instance().find_group_bus(config_addr)?;
        let handle = Arc::new(PciHandle::new(address)?);

        let mut handles = self.handles.write();
        handles.insert(*config_addr, Arc::clone(&handle));
        Ok(handle)
    }

    pub fn read32(&self, config_addr: &PciConfigAddress, offset: u32) -> Result<u32> {
        let handle = self.get_or_create_handle(config_addr)?;
        handle.read32(offset)
    }

    pub fn write32(&self, config_addr: &PciConfigAddress, offset: u32, value: u32) -> Result<()> {
        let handle = self.get_or_create_handle(config_addr)?;
        handle.write32(offset, value)
    }

    pub fn read64(&self, config_addr: &PciConfigAddress, offset: u32) -> Result<u64> {
        let handle = self.get_or_create_handle(config_addr)?;
        handle.read64(offset)
    }
}

pub fn device_exists(group: u32, bus: u32, device: u32, function: u32) -> bool {
    let address = PciAddress {
        group_number: group,
        bus,
        device,
        function,
    };
    PciHandle::new(address).is_ok()
}
