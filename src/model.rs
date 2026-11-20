//! Disk and partition model shared by the window and the worker.

use serde::{Deserialize, Serialize};

pub const KIB: u64 = 1024;
pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;
pub const TIB: u64 = 1024 * GIB;

/// Bytes reserved at the start of a GPT disk: (bps + 0x43FF) / bps sectors (34 at 512 B).
pub fn gpt_head_sectors(sector: u64) -> u64 {
    (sector + 0x43FF) / sector
}
/// Sectors used by the backup GPT at the end of the disk (entries + header): one less than the head.
pub fn gpt_tail_sectors(sector: u64) -> u64 {
    gpt_head_sectors(sector) - 1
}

/// Largest MBR partition end: 32-bit sector numbers.
pub fn mbr_max_bytes(sector: u64) -> u64 {
    (1u64 << 32) * sector
}

/// FAT32 limits.
pub const FAT32_MAX_FILE: u64 = 4 * GIB - 1;
pub const FAT32_MIN_CLUSTERS: u64 = 65525;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Style {
    Mbr,
    Gpt,
    Raw,
}

impl Style {
    pub fn name(&self) -> &'static str {
        match self {
            Style::Mbr => "MBR",
            Style::Gpt => "GPT",
            Style::Raw => crate::tr!("Non initialisé", "Not initialized"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fs {
    Ntfs,
    Fat32,
    ExFat,
    Fat,
    Refs,
    Raw,
    Other(String),
}

impl Fs {
    pub fn parse(s: &str) -> Fs {
        match s.trim().to_ascii_uppercase().as_str() {
            "NTFS" => Fs::Ntfs,
            "FAT32" => Fs::Fat32,
            "EXFAT" => Fs::ExFat,
            "FAT" | "FAT16" | "FAT12" => Fs::Fat,
            "REFS" => Fs::Refs,
            "" | "UNKNOWN" | "RAW" => Fs::Raw,
            other => Fs::Other(other.to_string()),
        }
    }
    pub fn name(&self) -> String {
        match self {
            Fs::Ntfs => "NTFS".into(),
            Fs::Fat32 => "FAT32".into(),
            Fs::ExFat => "exFAT".into(),
            Fs::Fat => "FAT".into(),
            Fs::Refs => "ReFS".into(),
            Fs::Raw => crate::tr!("Non formatée", "Unformatted").into(),
            Fs::Other(s) => s.clone(),
        }
    }
    /// Name used by Format-Volume.
    pub fn ps_name(&self) -> &'static str {
        match self {
            Fs::Fat32 => "FAT32",
            Fs::ExFat => "exFAT",
            Fs::Fat => "FAT",
            Fs::Refs => "ReFS",
            _ => "NTFS",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Data,
    Efi,
    Msr,
    Recovery,
    LdmMeta,
    LdmData,
    Extended,
    Logical,
}

impl Kind {
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Data => crate::tr!("Principale", "Primary"),
            Kind::Efi => crate::tr!("Système EFI", "EFI system"),
            Kind::Msr => crate::tr!("Réservée Microsoft", "Microsoft reserved"),
            Kind::Recovery => crate::tr!("Récupération", "Recovery"),
            Kind::LdmMeta => crate::tr!("Métadonnées dynamiques", "Dynamic metadata"),
            Kind::LdmData => crate::tr!("Volume dynamique", "Dynamic volume"),
            Kind::Extended => crate::tr!("Étendue", "Extended"),
            Kind::Logical => crate::tr!("Logique", "Logical"),
        }
    }
}

pub const GUID_BASIC: &str = "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7";
pub const GUID_EFI: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
pub const GUID_MSR: &str = "E3C9E316-0B5C-4DB8-817D-F92DF00215AE";
pub const GUID_RECOVERY: &str = "DE94BBA4-06D1-4D40-A16A-BFD50179D6AC";
pub const GUID_LDM_META: &str = "5808C8AA-7E8F-42E0-85D2-E1E90434CFB3";
pub const GUID_LDM_DATA: &str = "AF9B60A0-1431-4F62-BC68-3311714A69AD";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Partition {
    pub number: u32,
    pub offset: u64,
    pub size: u64,
    pub letter: Option<char>,
    pub fs: Fs,
    pub label: String,
    pub used: Option<u64>,
    pub kind: Kind,
    pub is_system: bool,
    pub is_boot: bool,
    pub is_active: bool,
    pub hidden: bool,
    /// Changed by a pending operation (drawn striped).
    #[serde(default)]
    pub pending: bool,
}

impl Partition {
    pub fn end(&self) -> u64 {
        self.offset + self.size
    }
    pub fn title(&self) -> String {
        let l = match self.letter {
            Some(c) => format!("{c}:"),
            None => "*:".to_string(),
        };
        if self.label.is_empty() {
            l
        } else {
            format!("{l} {}", self.label)
        }
    }
    pub fn free(&self) -> Option<u64> {
        self.used.map(|u| self.size.saturating_sub(u))
    }
    /// Windows refuses to delete, wipe or format the running system.
    pub fn protected(&self) -> bool {
        self.is_system || self.is_boot
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disk {
    pub number: u32,
    pub model: String,
    pub size: u64,
    pub sector: u64,
    pub style: Style,
    pub dynamic: bool,
    pub is_system: bool,
    pub is_boot: bool,
    pub bus: String,
    pub offline: bool,
    /// Windows unique id of the disk (serial number based); lets the worker recognise it again.
    #[serde(default)]
    pub uid: String,
    pub parts: Vec<Partition>,
    #[serde(default)]
    pub pending: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Part(usize),
    Free { offset: u64, size: u64 },
}

impl Disk {
    pub fn usable_start(&self) -> u64 {
        match self.style {
            Style::Gpt => gpt_head_sectors(self.sector) * self.sector,
            _ => self.sector,
        }
    }
    pub fn usable_end(&self) -> u64 {
        match self.style {
            Style::Gpt => self.size - gpt_tail_sectors(self.sector) * self.sector,
            _ => self.size,
        }
    }
    /// Partitions (not the extended container) sorted by offset, with free gaps of 1 MiB or more.
    pub fn segments(&self) -> Vec<Segment> {
        let mut idx: Vec<usize> = (0..self.parts.len())
            .filter(|&i| self.parts[i].kind != Kind::Extended)
            .collect();
        idx.sort_by_key(|&i| self.parts[i].offset);
        let mut out = Vec::new();
        let mut pos = self.usable_start();
        for i in idx {
            let p = &self.parts[i];
            if p.offset > pos && align_down(p.offset, MIB) > align_up(pos, MIB) + MIB {
                out.push(Segment::Free { offset: align_up(pos, MIB), size: align_down(p.offset, MIB) - align_up(pos, MIB) });
            }
            out.push(Segment::Part(i));
            pos = pos.max(p.end());
        }
        let end = self.usable_end();
        if end > pos && align_down(end, MIB) > align_up(pos, MIB) + MIB {
            out.push(Segment::Free { offset: align_up(pos, MIB), size: align_down(end, MIB) - align_up(pos, MIB) });
        }
        out
    }
    pub fn part_at(&self, offset: u64) -> Option<usize> {
        self.parts.iter().position(|p| p.offset == offset && p.kind != Kind::Extended)
    }
    /// Free space directly after a partition (contiguous).
    pub fn free_after(&self, offset: u64) -> u64 {
        let Some(i) = self.part_at(offset) else { return 0 };
        let end = self.parts[i].end();
        let next = self
            .parts
            .iter()
            .filter(|p| p.offset >= end && p.kind != Kind::Extended)
            .map(|p| p.offset)
            .min()
            .unwrap_or(self.usable_end());
        next.saturating_sub(end)
    }
    /// Free space directly before a partition, from the first 1 MiB-aligned byte after the
    /// previous partition (or the start of the usable area).
    pub fn free_before(&self, offset: u64) -> u64 {
        let prev = self
            .parts
            .iter()
            .filter(|p| p.end() <= offset && p.kind != Kind::Extended)
            .map(|p| p.end())
            .max()
            .unwrap_or_else(|| self.usable_start());
        let start = align_up(prev, MIB);
        offset.saturating_sub(start)
    }
    pub fn next_part(&self, offset: u64) -> Option<usize> {
        let i = self.part_at(offset)?;
        let end = self.parts[i].end();
        self.parts
            .iter()
            .enumerate()
            .filter(|(_, p)| p.offset >= end && p.kind != Kind::Extended)
            .min_by_key(|(_, p)| p.offset)
            .map(|(j, _)| j)
    }
    /// End of the last partition (what a clone must copy).
    pub fn used_extent(&self) -> u64 {
        self.parts.iter().map(|p| p.end()).max().unwrap_or(0)
    }
    pub fn type_name(&self) -> String {
        format!("{} · {}", self.style.name(), if self.dynamic { crate::tr!("Dynamique", "Dynamic") } else { crate::tr!("De base", "Basic") })
    }
    pub fn primary_count(&self) -> usize {
        self.parts.iter().filter(|p| p.kind != Kind::Logical).count()
    }
}

pub fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}
pub fn align_down(v: u64, a: u64) -> u64 {
    v / a * a
}

/// Size text: one decimal, 1024-based, TB if at least 1.0 TB, else GB if at least 1.0 GB,
/// else MB, trailing ",0" removed.
pub fn fmt_size(bytes: u64) -> String {
    let mb = bytes as f64 / MIB as f64;
    let gb = mb / 1024.0;
    let tb = gb / 1024.0;
    let (v, unit) = if tb >= 1.0 {
        (tb, crate::tr!("To", "TB"))
    } else if gb >= 1.0 {
        (gb, crate::tr!("Go", "GB"))
    } else if mb >= 1.0 {
        (mb, crate::tr!("Mo", "MB"))
    } else {
        (bytes as f64 / KIB as f64, crate::tr!("Ko", "KB"))
    };
    let s = format!("{v:.1}");
    let s = s.strip_suffix(".0").unwrap_or(&s).to_string();
    // French writes 212,6 Go; English 212.6 GB.
    let s = if crate::i18n::en() { s } else { s.replace('.', ",") };
    format!("{s} {unit}")
}

/// Health of a physical disk, as Windows reports it (storage reliability counters and the
/// drive's own SMART failure prediction). Every value may be missing: many USB enclosures and
/// some drivers report nothing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Health {
    /// Healthy, Warning, Unhealthy or Unknown (Windows' own verdict).
    pub status: String,
    pub operational: String,
    /// SSD, HDD, SCM or Unspecified.
    pub media: String,
    pub bus: String,
    pub firmware: String,
    pub serial: String,
    /// Degrees Celsius.
    pub temperature: Option<u32>,
    pub temperature_max: Option<u32>,
    /// Percentage of the rated endurance already used (SSD).
    pub wear: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub read_errors: Option<u64>,
    pub read_errors_uncorrected: Option<u64>,
    pub write_errors: Option<u64>,
    pub write_errors_uncorrected: Option<u64>,
    pub start_stop_cycles: Option<u64>,
    pub load_unload_cycles: Option<u64>,
    /// The drive's SMART "failure predicted" flag (SATA/SAS drives).
    pub predict_failure: Option<bool>,
    // Read straight from the drive (NVMe health log or ATA SMART attributes).
    pub available_spare: Option<u32>,
    pub available_spare_threshold: Option<u32>,
    /// NVMe critical warning bits (0 = none).
    pub critical_warning: Option<u8>,
    pub power_cycles: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub media_errors: Option<u64>,
    pub error_log_entries: Option<u64>,
    /// Bytes read / written over the drive's life.
    pub data_read: Option<u64>,
    pub data_written: Option<u64>,
    pub reallocated: Option<u64>,
    pub pending: Option<u64>,
    pub uncorrectable: Option<u64>,
    pub crc_errors: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HealthLevel {
    Good,
    Warning,
    Bad,
    Unknown,
}

impl Health {
    /// Overall verdict: the worst of Windows' status, the SMART prediction and the counters.
    pub fn level(&self) -> HealthLevel {
        let uncorrected = self.read_errors_uncorrected.unwrap_or(0) + self.write_errors_uncorrected.unwrap_or(0) + self.uncorrectable.unwrap_or(0) + self.pending.unwrap_or(0);
        let spare_low = matches!((self.available_spare, self.available_spare_threshold), (Some(a), Some(t)) if t > 0 && a < t);
        if self.status.eq_ignore_ascii_case("Unhealthy") || self.predict_failure == Some(true) || self.critical_warning.is_some_and(|c| c != 0) || uncorrected > 0 || spare_low {
            HealthLevel::Bad
        } else if self.status.eq_ignore_ascii_case("Warning")
            || self.wear.is_some_and(|w| w >= 90)
            || self.temperature.is_some_and(|t| t >= 70)
            || self.reallocated.is_some_and(|r| r > 0)
            || self.media_errors.is_some_and(|m| m > 0)
        {
            HealthLevel::Warning
        } else if self.status.eq_ignore_ascii_case("Healthy") {
            HealthLevel::Good
        } else {
            HealthLevel::Unknown
        }
    }
    /// Whether any SMART counter was reported (Windows hides them without administrator rights).
    pub fn has_counters(&self) -> bool {
        self.temperature.is_some() || self.wear.is_some() || self.power_on_hours.is_some() || self.read_errors.is_some() || self.write_errors.is_some() || self.power_cycles.is_some()
    }
}
