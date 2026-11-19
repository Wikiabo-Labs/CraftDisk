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
            Style::Raw => "Non initialisé",
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
            Fs::Raw => "Non formatée".into(),
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
            Kind::Data => "Principale",
            Kind::Efi => "Système EFI",
            Kind::Msr => "Réservée Microsoft",
            Kind::Recovery => "Récupération",
            Kind::LdmMeta => "Métadonnées dynamiques",
            Kind::LdmData => "Volume dynamique",
            Kind::Extended => "Étendue",
            Kind::Logical => "Logique",
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
        format!("{} · {}", self.style.name(), if self.dynamic { "Dynamique" } else { "De base" })
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
        (tb, "To")
    } else if gb >= 1.0 {
        (gb, "Go")
    } else if mb >= 1.0 {
        (mb, "Mo")
    } else {
        (bytes as f64 / KIB as f64, "Ko")
    };
    let s = format!("{v:.1}");
    let s = s.strip_suffix(".0").unwrap_or(&s).replace('.', ",");
    format!("{s} {unit}")
}
