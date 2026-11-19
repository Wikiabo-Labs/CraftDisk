//! The steps the elevated worker carries out, written to a JSON file by the window.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SrcPart {
    pub offset: u64,
    pub size: u64,
    pub letter: Option<char>,
    pub ntfs: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Step {
    /// A PowerShell script (storage cmdlets, robocopy, diskpart...).
    Ps { script: String },
    /// Copy a whole disk sector by sector; mounted NTFS volumes are read from a snapshot.
    CloneDisk { src: u32, dst: u32, sector: u64, parts: Vec<SrcPart>, extent: u64 },
    /// Copy one partition into an already created partition at dst_offset.
    ClonePart { src_disk: u32, part: SrcPart, dst_disk: u32, dst_offset: u64 },
    /// Give a disk new identifiers and move the backup GPT to its real end.
    FixIds { disk: u32, sector: u64 },
    /// Rebuild the boot files on the new disk (bcdboot, plus bootsect for BIOS).
    FixBoot { disk: u32, win_offset: u64, sys_offset: u64, uefi: bool },
    /// Move a partition's data to a new place on the same disk and update the partition table.
    MovePart { disk: u32, offset: u64, size: u64, new_offset: u64, sector: u64 },
    /// Overwrite a whole disk or one partition.
    WipeDisk { disk: u32, passes: u8 },
    WipePart { disk: u32, offset: u64, passes: u8 },
    /// Rewrite the partition table in the other style, keeping the data.
    ToGpt { disk: u32, sector: u64 },
    ToMbr { disk: u32, sector: u64, fat32_offsets: Vec<u64> },
    /// Write a FAT32 file system (any size up to 2 TiB).
    FormatFat32 { disk: u32, offset: u64, label: String, cluster: u32 },
    /// Read every sector of a range and report the unreadable ones.
    SurfaceTest { disk: u32, offset: u64, size: u64, sector: u64 },
}

impl Step {
    pub fn title(&self) -> String {
        match self {
            Step::Ps { script } => {
                let first = script.lines().find(|l| l.trim_start().starts_with("# ")).unwrap_or("# Commande");
                first.trim_start().trim_start_matches("# ").to_string()
            }
            Step::CloneDisk { src, dst, .. } => format!("Copie du disque {src} vers le disque {dst}"),
            Step::ClonePart { src_disk, dst_disk, .. } => format!("Copie d'une partition du disque {src_disk} vers le disque {dst_disk}"),
            Step::MovePart { disk, .. } => format!("Déplacement d'une partition sur le disque {disk}"),
            Step::FixIds { disk, .. } => format!("Nouveaux identifiants pour le disque {disk}"),
            Step::FixBoot { disk, .. } => format!("Réparation du démarrage sur le disque {disk}"),
            Step::WipeDisk { disk, passes } => format!("Effacement du disque {disk} ({passes} passe(s))"),
            Step::WipePart { disk, passes, .. } => format!("Effacement d'une partition du disque {disk} ({passes} passe(s))"),
            Step::ToGpt { disk, .. } => format!("Conversion du disque {disk} en GPT"),
            Step::ToMbr { disk, .. } => format!("Conversion du disque {disk} en MBR"),
            Step::FormatFat32 { disk, .. } => format!("Formatage FAT32 sur le disque {disk}"),
            Step::SurfaceTest { disk, .. } => format!("Test de surface sur le disque {disk}"),
        }
    }
}

/// How a disk looked when the plan was made: the worker checks it again before writing anything,
/// because disk numbers can change (USB unplugged and plugged back, new disk...).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiskId {
    pub number: u32,
    pub size: u64,
    pub uid: String,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub disks: Vec<DiskId>,
}
