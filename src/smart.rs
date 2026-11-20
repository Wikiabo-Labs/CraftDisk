//! SMART values read straight from the drive (administrator rights needed): the NVMe
//! "SMART / Health Information" log page, or the ATA SMART attribute table for SATA drives.
//! Windows' own reliability counters often leave these empty, NVMe drives especially.

use crate::rawio::{ioctl_inout, Dev};

const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
const SMART_RCV_DRIVE_DATA: u32 = 0x0007_C088;

/// What the drive itself reports. Every field is optional: drives and USB bridges vary.
#[derive(Clone, Debug, Default)]
pub struct Smart {
    pub temperature: Option<u32>,
    pub wear: Option<u32>,
    pub available_spare: Option<u32>,
    pub available_spare_threshold: Option<u32>,
    pub critical_warning: Option<u8>,
    pub power_on_hours: Option<u64>,
    pub power_cycles: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub media_errors: Option<u64>,
    pub error_log_entries: Option<u64>,
    pub data_read: Option<u64>,
    pub data_written: Option<u64>,
    pub start_stop: Option<u64>,
    pub load_unload: Option<u64>,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc_errors: Option<u64>,
    /// An ATA attribute marked "pre-failure" is at or below its threshold.
    pub threshold_exceeded: Option<bool>,
}

fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap_or([0; 8]))
}

/// Reads the SMART values of physical disk `n` (NVMe first, then ATA). None if neither answers.
pub fn read(n: u32) -> Option<Smart> {
    let dev = Dev::open(&format!(r"\\.\PhysicalDrive{n}"), false).ok()?;
    nvme(&dev).or_else(|| ata(&dev, n))
}

/// NVMe log page 0x02 through IOCTL_STORAGE_QUERY_PROPERTY (StorageDeviceProtocolSpecificProperty).
fn nvme(dev: &Dev) -> Option<Smart> {
    const HEADER: usize = 8; // STORAGE_PROPERTY_QUERY: PropertyId, QueryType
    const SPECIFIC: usize = 40; // STORAGE_PROTOCOL_SPECIFIC_DATA
    const LOG_LEN: usize = 512; // NVME_HEALTH_INFO_LOG
    let mut buf = vec![0u8; HEADER + SPECIFIC + LOG_LEN];
    let put = |b: &mut [u8], o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut buf, 0, 50); // StorageDeviceProtocolSpecificProperty
    put(&mut buf, 4, 0); // PropertyStandardQuery
    put(&mut buf, 8, 3); // ProtocolTypeNvme
    put(&mut buf, 12, 2); // NVMeDataTypeLogPage
    put(&mut buf, 16, 0x02); // SMART / Health Information log
    put(&mut buf, 20, 0);
    put(&mut buf, 24, SPECIFIC as u32); // ProtocolDataOffset
    put(&mut buf, 28, LOG_LEN as u32); // ProtocolDataLength
    let len = buf.len();
    ioctl_inout(&dev.file, IOCTL_STORAGE_QUERY_PROPERTY, &mut buf, len).ok()?;
    // Output: STORAGE_PROTOCOL_DATA_DESCRIPTOR = Version, Size, then the protocol data header.
    let off = u32::from_le_bytes(buf[8 + 16..8 + 20].try_into().ok()?) as usize;
    let dlen = u32::from_le_bytes(buf[8 + 20..8 + 24].try_into().ok()?) as usize;
    let start = 8 + off;
    if off == 0 || dlen < 192 || start + 192 > buf.len() {
        return None;
    }
    let l = &buf[start..];
    let kelvin = u16::from_le_bytes([l[1], l[2]]) as u32;
    // Data units are thousands of 512-byte blocks.
    let units = |o: usize| le64(l, o).saturating_mul(512_000);
    Some(Smart {
        critical_warning: Some(l[0]),
        temperature: (kelvin > 273).then(|| kelvin - 273),
        available_spare: Some(l[3] as u32),
        available_spare_threshold: Some(l[4] as u32),
        wear: Some(l[5] as u32),
        data_read: Some(units(32)),
        data_written: Some(units(48)),
        power_cycles: Some(le64(l, 112)),
        power_on_hours: Some(le64(l, 128)),
        unsafe_shutdowns: Some(le64(l, 144)),
        media_errors: Some(le64(l, 160)),
        error_log_entries: Some(le64(l, 176)),
        ..Default::default()
    })
}

/// One 512-byte SMART data page (feature 0xD0 attributes, 0xD1 thresholds) via SMART_RCV_DRIVE_DATA.
fn ata_page(dev: &Dev, n: u32, feature: u8) -> Option<Vec<u8>> {
    // SENDCMDINPARAMS (packed, 32 bytes used) followed by room for SENDCMDOUTPARAMS (16 + 512).
    let mut buf = vec![0u8; 16 + 512];
    buf[0..4].copy_from_slice(&512u32.to_le_bytes()); // cBufferSize
    buf[4] = feature; // bFeaturesReg
    buf[5] = 1; // bSectorCountReg
    buf[6] = 1; // bSectorNumberReg
    buf[7] = 0x4F; // bCylLowReg
    buf[8] = 0xC2; // bCylHighReg
    buf[9] = 0xA0 | (((n & 1) as u8) << 4); // bDriveHeadReg
    buf[10] = 0xB0; // bCommandReg: SMART
    buf[12] = n as u8; // bDriveNumber
    ioctl_inout(&dev.file, SMART_RCV_DRIVE_DATA, &mut buf, 32).ok()?;
    // SENDCMDOUTPARAMS: cBufferSize (4), DRIVERSTATUS (12), then the data.
    if buf[4] != 0 {
        return None; // bDriverError
    }
    Some(buf[16..16 + 512].to_vec())
}

fn ata(dev: &Dev, n: u32) -> Option<Smart> {
    let attrs = ata_page(dev, n, 0xD0)?;
    let thresholds = ata_page(dev, n, 0xD1);
    let mut s = Smart::default();
    let mut exceeded = false;
    let mut any = false;
    for i in 0..30 {
        let e = &attrs[2 + i * 12..2 + i * 12 + 12];
        let id = e[0];
        if id == 0 {
            continue;
        }
        any = true;
        let flags = u16::from_le_bytes([e[1], e[2]]);
        let current = e[3];
        let raw = u64::from_le_bytes([e[5], e[6], e[7], e[8], e[9], e[10], 0, 0]);
        if let Some(t) = &thresholds {
            // Threshold table: same layout, id then threshold, 12 bytes per entry.
            if let Some(te) = (0..30).map(|j| &t[2 + j * 12..2 + j * 12 + 12]).find(|te| te[0] == id) {
                let th = te[1];
                if flags & 1 != 0 && th != 0 && current != 0 && current <= th {
                    exceeded = true;
                }
            }
        }
        match id {
            4 => s.start_stop = Some(raw & 0xFFFF_FFFF),
            5 => s.reallocated = Some(raw & 0xFFFF_FFFF),
            9 => s.power_on_hours = Some(raw & 0xFFFF_FFFF),
            12 => s.power_cycles = Some(raw & 0xFFFF_FFFF),
            192 => s.unsafe_shutdowns = Some(raw & 0xFFFF_FFFF),
            193 => s.load_unload = Some(raw & 0xFFFF_FFFF),
            194 => s.temperature = Some((raw & 0xFF) as u32).filter(|&t| t > 0 && t < 120),
            197 => s.pending = Some(raw & 0xFFFF_FFFF),
            198 => s.uncorrectable = Some(raw & 0xFFFF_FFFF),
            199 => s.crc_errors = Some(raw & 0xFFFF_FFFF),
            _ => {}
        }
    }
    if !any {
        return None;
    }
    s.threshold_exceeded = thresholds.map(|_| exceeded);
    Some(s)
}
