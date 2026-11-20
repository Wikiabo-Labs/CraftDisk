//! Pending operations: each one checks its rules, changes the preview of the disks
//! and turns into worker steps when the person presses "Exécuter".

use crate::model::*;
use crate::plan::{SrcPart, Step};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Op {
    /// Resize and/or move a partition inside the free space around it.
    Resize { disk: u32, offset: u64, new_offset: u64, new_size: u64 },
    Create { disk: u32, offset: u64, size: u64, fs: Fs, label: String, letter: Option<char> },
    Format { disk: u32, offset: u64, fs: Fs, label: String, cluster: u32, quick: bool },
    Delete { disk: u32, offset: u64 },
    WipePart { disk: u32, offset: u64, passes: u8 },
    WipeDisk { disk: u32, passes: u8 },
    Merge { disk: u32, first: u64 },
    SetLetter { disk: u32, offset: u64, letter: Option<char> },
    SetLabel { disk: u32, offset: u64, label: String },
    SetActive { disk: u32, offset: u64 },
    ClonePart { src_disk: u32, src_offset: u64, dst_disk: u32, dst_offset: u64 },
    CloneDisk { src: u32, dst: u32, fit: bool },
    MigrateOs { src: u32, dst: u32, fit: bool },
    ToGpt { disk: u32 },
    ToMbr { disk: u32 },
    NtfsToFat32 { disk: u32, offset: u64, temp: char },
    DynamicToBasic { disk: u32, temp: char },
}

pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn disk_mut(disks: &mut [Disk], n: u32) -> Result<&mut Disk, String> {
    disks.iter_mut().find(|d| d.number == n).ok_or_else(|| crate::trf!("Le disque {n} n'existe plus.", "Disk {n} no longer exists."))
}
pub fn disk_ref(disks: &[Disk], n: u32) -> Result<&Disk, String> {
    disks.iter().find(|d| d.number == n).ok_or_else(|| crate::trf!("Le disque {n} n'existe plus.", "Disk {n} no longer exists."))
}
fn part_of(d: &Disk, offset: u64) -> Result<usize, String> {
    d.part_at(offset).ok_or_else(|| crate::tr!("Cette partition n'existe plus (une autre opération l'a changée).", "This partition no longer exists (another operation changed it).").to_string())
}

/// Smallest size a partition can be shrunk to: used space plus 10 % (estimated margin).
pub fn min_size(p: &Partition) -> u64 {
    let used = p.used.unwrap_or(p.size);
    align_up((used + used / 10).max(100 * MIB), MIB).min(p.size)
}

/// Default FAT32 cluster size for a volume size (Microsoft's table).
pub fn fat32_default_cluster(size: u64) -> u32 {
    match size {
        s if s <= 64 * MIB => 512,
        s if s <= 128 * MIB => 1024,
        s if s <= 256 * MIB => 2048,
        s if s <= 8 * GIB => 4096,
        s if s <= 16 * GIB => 8192,
        s if s <= 32 * GIB => 16384,
        _ => 32768,
    }
}

impl Op {
    pub fn describe(&self, disks: &[Disk]) -> String {
        let pname = |d: u32, o: u64| -> String {
            disk_ref(disks, d)
                .ok()
                .and_then(|dk| dk.part_at(o).map(|i| dk.parts[i].title()))
                .unwrap_or_else(|| "partition".into())
        };
        match self {
            Op::Resize { disk, offset, new_offset, new_size } => {
                if new_offset != offset {
                    let dir = if new_offset < offset { crate::tr!("vers la gauche", "to the left") } else { crate::tr!("vers la droite", "to the right") };
                    crate::trf!("Déplacer {} de {} {dir}, taille {} (disque {disk})", "Move {} by {} {dir}, size {} (disk {disk})", pname(*disk, *offset), fmt_size(new_offset.abs_diff(*offset)), fmt_size(*new_size))
                } else {
                    crate::trf!("Redimensionner {} à {} (disque {disk})", "Resize {} to {} (disk {disk})", pname(*disk, *offset), fmt_size(*new_size))
                }
            }
            Op::Create { disk, size, fs, .. } => crate::trf!("Créer une partition {} de {} sur le disque {disk}", "Create a {} partition of {} on disk {disk}", fs.name(), fmt_size(*size)),
            Op::Format { disk, offset, fs, .. } => crate::trf!("Formater {} en {} (disque {disk})", "Format {} as {} (disk {disk})", pname(*disk, *offset), fs.name()),
            Op::Delete { disk, offset } => crate::trf!("Supprimer {} (disque {disk})", "Delete {} (disk {disk})", pname(*disk, *offset)),
            Op::WipePart { disk, offset, passes } => crate::trf!("Effacer {} en {passes} passe(s) (disque {disk})", "Wipe {} with {passes} pass(es) (disk {disk})", pname(*disk, *offset)),
            Op::WipeDisk { disk, passes } => crate::trf!("Effacer tout le disque {disk} en {passes} passe(s)", "Wipe the whole disk {disk} with {passes} pass(es)"),
            Op::Merge { disk, first } => crate::trf!("Fusionner {} avec la partition suivante (disque {disk})", "Merge {} with the next partition (disk {disk})", pname(*disk, *first)),
            Op::SetLetter { disk, offset, letter } => match letter {
                Some(l) => crate::trf!("Donner la lettre {l}: à {} (disque {disk})", "Give letter {l}: to {} (disk {disk})", pname(*disk, *offset)),
                None => crate::trf!("Masquer {} (retirer sa lettre)", "Hide {} (remove its letter)", pname(*disk, *offset)),
            },
            Op::SetLabel { disk, offset, label } => crate::trf!("Renommer {} en « {label} » (disque {disk})", "Rename {} to \"{label}\" (disk {disk})", pname(*disk, *offset)),
            Op::SetActive { disk, offset } => crate::trf!("Rendre active {} (disque {disk})", "Set {} active (disk {disk})", pname(*disk, *offset)),
            Op::ClonePart { src_disk, src_offset, dst_disk, .. } => crate::trf!("Cloner {} (disque {src_disk}) vers le disque {dst_disk}", "Clone {} (disk {src_disk}) to disk {dst_disk}", pname(*src_disk, *src_offset)),
            Op::CloneDisk { src, dst, .. } => crate::trf!("Cloner le disque {src} vers le disque {dst}", "Clone disk {src} to disk {dst}"),
            Op::MigrateOs { src, dst, .. } => crate::trf!("Migrer Windows du disque {src} vers le disque {dst}", "Migrate Windows from disk {src} to disk {dst}"),
            Op::ToGpt { disk } => crate::trf!("Convertir le disque {disk} en GPT", "Convert disk {disk} to GPT"),
            Op::ToMbr { disk } => crate::trf!("Convertir le disque {disk} en MBR", "Convert disk {disk} to MBR"),
            Op::NtfsToFat32 { disk, offset, temp } => crate::trf!("Convertir {} de NTFS en FAT32 (copie temporaire sur {temp}:) (disque {disk})", "Convert {} from NTFS to FAT32 (temporary copy on {temp}:) (disk {disk})", pname(*disk, *offset)),
            Op::DynamicToBasic { disk, temp } => crate::trf!("Convertir le disque dynamique {disk} en disque de base (copie temporaire sur {temp}:)", "Convert dynamic disk {disk} to a basic disk (temporary copy on {temp}:)"),
        }
    }

    /// Checks the rules and changes the preview. `disks` is the preview before this operation.
    pub fn apply(&self, disks: &mut Vec<Disk>) -> Result<(), String> {
        match self {
            Op::Resize { disk, offset, new_offset, new_size } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let lo = *offset - d.free_before(*offset);
                let hi = d.parts[i].end() + d.free_after(*offset);
                let p = &d.parts[i];
                let moved = new_offset != offset;
                let resized = *new_size != p.size;
                if !moved && !resized {
                    return Err(crate::tr!("Rien n'a changé.", "Nothing changed.").into());
                }
                if resized && !matches!(p.fs, Fs::Ntfs | Fs::Refs) {
                    return Err(crate::tr!("Veuillez convertir la partition en NTFS, puis réessayer (Windows ne redimensionne que NTFS). Elle peut quand même être déplacée sans changer de taille.", "Please convert the partition to NTFS, then try again (Windows only resizes NTFS). It can still be moved without changing its size.").into());
                }
                if *new_size < p.size && p.fs != Fs::Ntfs {
                    return Err(crate::tr!("Seules les partitions NTFS peuvent être réduites.", "Only NTFS partitions can be shrunk.").into());
                }
                if resized && *new_size < min_size(p) {
                    return Err(crate::trf!("La taille minimale est {}.", "The minimum size is {}.", fmt_size(min_size(p))));
                }
                let in_own = *new_offset >= lo && *new_offset + *new_size <= hi;
                // Or anywhere else on the disk, inside one free area (the partition jumps over its neighbours).
                let in_other = moved
                    && d.segments().iter().any(|s| matches!(s, Segment::Free { offset: o, size: z } if *new_offset >= *o && *new_offset + *new_size <= *o + *z));
                if !in_own && !in_other {
                    return Err(crate::tr!("Pas assez d'espace non alloué à cet endroit pour la partition sélectionnée.", "Not enough unallocated space at this place for the selected partition.").into());
                }
                if moved {
                    if p.protected() {
                        return Err(crate::tr!("Windows utilise cette partition en ce moment : pour la déplacer, démarrez le PC sur le disque de démarrage DiskCraft (WinPE).", "Windows is using this partition right now: to move it, start the PC from the DiskCraft boot disk (WinPE).").into());
                    }
                    if matches!(p.kind, Kind::Logical | Kind::Extended | Kind::LdmData | Kind::LdmMeta) {
                        return Err(crate::tr!("Les partitions logiques et dynamiques ne peuvent pas être déplacées.", "Logical and dynamic partitions cannot be moved.").into());
                    }
                    if new_offset % MIB != 0 {
                        return Err(crate::tr!("La nouvelle position doit être alignée sur 1 Mo.", "The new position must be aligned to 1 MB.").into());
                    }
                    if d.style == Style::Mbr && d.offset_end_ok(*new_offset + *new_size).is_err() {
                        return Err(crate::tr!("Sur un disque MBR, une partition ne peut pas dépasser 2 To depuis le début du disque.", "On an MBR disk, a partition cannot go beyond 2 TB from the start of the disk.").into());
                    }
                }
                let p = &mut d.parts[i];
                p.offset = *new_offset;
                p.size = *new_size;
                p.pending = true;
            }
            Op::Create { disk, offset, size, fs, label, letter } => {
                let d = disk_mut(disks, *disk)?;
                if d.style == Style::Raw {
                    return Err(crate::tr!("Initialisez d'abord le disque (Convertir en GPT ou MBR).", "Initialize the disk first (Convert to GPT or MBR).").into());
                }
                if d.dynamic {
                    return Err(crate::tr!("Convertissez d'abord le disque en disque de base.", "Convert the disk to a basic disk first.").into());
                }
                let free = d.segments().into_iter().find_map(|s| match s {
                    Segment::Free { offset: o, size: s } if o <= *offset && *offset + *size <= o + s => Some(()),
                    _ => None,
                });
                if free.is_none() || *size < MIB {
                    return Err(crate::tr!("Pas assez d'espace non alloué à cet endroit.", "Not enough unallocated space at this place.").into());
                }
                if *fs == Fs::Fat32 && *size > 2 * TIB {
                    return Err(crate::tr!("FAT32 est limité à 2 To.", "FAT32 is limited to 2 TB.").into());
                }
                if d.style == Style::Mbr && d.offset_end_ok(*offset + *size).is_err() {
                    return Err(crate::tr!("Sur un disque MBR, une partition ne peut pas dépasser 2 To depuis le début du disque.", "On an MBR disk, a partition cannot go beyond 2 TB from the start of the disk.").into());
                }
                let number = d.parts.iter().map(|p| p.number).max().unwrap_or(0) + 1;
                d.parts.push(Partition {
                    number,
                    offset: *offset,
                    size: *size,
                    letter: *letter,
                    fs: fs.clone(),
                    label: label.clone(),
                    used: Some(0),
                    kind: if d.style == Style::Mbr && d.primary_count() >= 4 { Kind::Logical } else { Kind::Data },
                    is_system: false,
                    is_boot: false,
                    is_active: false,
                    hidden: false,
                    pending: true,
                });
            }
            Op::Format { disk, offset, fs, label, .. } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &mut d.parts[i];
                if p.protected() {
                    return Err(crate::tr!("Impossible de formater la partition de Windows en cours d'utilisation.", "The running Windows partition cannot be formatted.").into());
                }
                if matches!(p.kind, Kind::LdmData | Kind::LdmMeta | Kind::Msr) {
                    return Err(crate::tr!("Cette partition ne peut pas être formatée ici.", "This partition cannot be formatted here.").into());
                }
                if *fs == Fs::Fat32 && p.size > 2 * TIB {
                    return Err(crate::tr!("FAT32 est limité à 2 To.", "FAT32 is limited to 2 TB.").into());
                }
                p.fs = fs.clone();
                p.label = label.clone();
                p.used = Some(0);
                p.pending = true;
            }
            Op::Delete { disk, offset } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &d.parts[i];
                if p.protected() {
                    return Err(crate::tr!("Impossible de supprimer une partition dont Windows a besoin pour démarrer.", "A partition Windows needs to start cannot be deleted.").into());
                }
                if matches!(p.kind, Kind::LdmData | Kind::LdmMeta) {
                    return Err(crate::tr!("Convertissez d'abord le disque dynamique en disque de base.", "Convert the dynamic disk to a basic disk first.").into());
                }
                d.parts.remove(i);
                d.pending = true;
            }
            Op::WipePart { disk, offset, passes } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &mut d.parts[i];
                if p.protected() {
                    return Err(crate::tr!("Impossible d'effacer la partition de Windows en cours d'utilisation.", "The running Windows partition cannot be wiped.").into());
                }
                if !(1..=10).contains(passes) {
                    return Err(crate::tr!("Choisissez de 1 à 10 passes.", "Choose from 1 to 10 passes.").into());
                }
                p.fs = Fs::Raw;
                p.label.clear();
                p.used = None;
                p.pending = true;
            }
            Op::WipeDisk { disk, passes } => {
                let d = disk_mut(disks, *disk)?;
                if d.is_system || d.is_boot {
                    return Err(crate::tr!("Impossible d'effacer le disque sur lequel Windows fonctionne.", "The disk Windows is running from cannot be wiped.").into());
                }
                if !(1..=10).contains(passes) {
                    return Err(crate::tr!("Choisissez de 1 à 10 passes.", "Choose from 1 to 10 passes.").into());
                }
                d.parts.clear();
                d.style = Style::Raw;
                d.dynamic = false;
                d.pending = true;
            }
            Op::Merge { disk, first } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *first)?;
                let j = d.next_part(*first).ok_or(crate::tr!("Il n'y a pas de partition après celle-ci.", "There is no partition after this one."))?;
                let (a, b) = (d.parts[i].clone(), d.parts[j].clone());
                if a.fs != Fs::Ntfs {
                    return Err(crate::tr!("Veuillez convertir la partition en NTFS, puis réessayer.", "Please convert the partition to NTFS, then try again.").into());
                }
                if a.letter.is_none() {
                    return Err(crate::tr!("La première partition doit avoir une lettre.", "The first partition must have a drive letter.").into());
                }
                if b.protected() || b.kind != Kind::Data && b.kind != Kind::Logical {
                    return Err(crate::tr!("La partition suivante ne peut pas être fusionnée (partition système ou spéciale).", "The next partition cannot be merged (system or special partition).").into());
                }
                if b.letter.is_none() && b.fs != Fs::Raw {
                    return Err(crate::tr!("Donnez d'abord une lettre à la partition suivante pour que son contenu soit copié.", "Give the next partition a letter first, so its contents can be copied.").into());
                }
                let need = b.used.unwrap_or(0);
                if need > a.free().unwrap_or(0) {
                    return Err(crate::tr!("Pas assez d'espace libre dans la première partition pour accueillir le contenu de la seconde.", "Not enough free space in the first partition for the contents of the second.").into());
                }
                let p = &mut d.parts[i];
                p.size = b.end() - a.offset;
                p.used = Some(a.used.unwrap_or(0) + need);
                p.pending = true;
                d.parts.remove(j);
            }
            Op::SetLetter { disk, offset, letter } => {
                let taken: Vec<char> = disks.iter().flat_map(|d| d.parts.iter().filter_map(|p| p.letter)).collect();
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &mut d.parts[i];
                if p.is_boot {
                    return Err(crate::tr!("La lettre du disque de Windows ne peut pas être changée.", "The Windows drive letter cannot be changed.").into());
                }
                if let Some(l) = letter {
                    if taken.contains(l) && p.letter != Some(*l) {
                        return Err(crate::trf!("La lettre {l}: est déjà utilisée.", "Letter {l}: is already in use."));
                    }
                }
                p.letter = *letter;
                p.pending = true;
            }
            Op::SetLabel { disk, offset, label } => {
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &mut d.parts[i];
                if p.fs == Fs::Raw {
                    return Err(crate::tr!("Formatez d'abord la partition.", "Format the partition first.").into());
                }
                let max = if matches!(p.fs, Fs::Fat32 | Fs::Fat) { 11 } else { 32 };
                if label.chars().count() > max {
                    return Err(crate::trf!("Le nom est limité à {max} caractères pour {}.", "The label is limited to {max} characters for {}.", p.fs.name()));
                }
                p.label = label.clone();
                p.pending = true;
            }
            Op::SetActive { disk, offset } => {
                let d = disk_mut(disks, *disk)?;
                if d.style != Style::Mbr {
                    return Err(crate::tr!("Seules les partitions d'un disque MBR peuvent être actives.", "Only partitions on an MBR disk can be active.").into());
                }
                let i = part_of(d, *offset)?;
                if d.parts[i].kind == Kind::Logical {
                    return Err(crate::tr!("Une partition logique ne peut pas être active.", "A logical partition cannot be active.").into());
                }
                for p in d.parts.iter_mut() {
                    p.is_active = false;
                }
                d.parts[i].is_active = true;
                d.parts[i].pending = true;
            }
            Op::ClonePart { src_disk, src_offset, dst_disk, dst_offset } => {
                let src = disk_ref(disks, *src_disk)?;
                let sp = src.parts[part_of(src, *src_offset)?].clone();
                if matches!(sp.kind, Kind::LdmData | Kind::LdmMeta | Kind::Extended) {
                    return Err(crate::tr!("Cette partition ne peut pas être clonée.", "This partition cannot be cloned.").into());
                }
                let d = disk_mut(disks, *dst_disk)?;
                if d.style == Style::Raw || d.dynamic {
                    return Err(crate::tr!("Le disque cible doit être un disque de base initialisé.", "The target disk must be an initialized basic disk.").into());
                }
                let fits = d.segments().into_iter().any(|s| matches!(s, Segment::Free { offset, size } if offset == *dst_offset && size >= sp.size));
                if !fits {
                    return Err(crate::tr!("Pas assez d'espace libre pour cloner cette partition.", "Not enough free space to clone this partition.").into());
                }
                if d.style == Style::Mbr && d.offset_end_ok(*dst_offset + sp.size).is_err() {
                    return Err(crate::tr!("Sur un disque MBR, la copie dépasserait la limite de 2 To.", "On an MBR disk, the copy would go beyond the 2 TB limit.").into());
                }
                let number = d.parts.iter().map(|p| p.number).max().unwrap_or(0) + 1;
                d.parts.push(Partition {
                    number,
                    offset: *dst_offset,
                    letter: None,
                    is_system: false,
                    is_boot: false,
                    is_active: false,
                    kind: Kind::Data,
                    pending: true,
                    ..sp
                });
            }
            Op::CloneDisk { src, dst, fit } | Op::MigrateOs { src, dst, fit } => {
                if src == dst {
                    return Err(crate::tr!("Choisissez deux disques différents.", "Choose two different disks.").into());
                }
                let s = disk_ref(disks, *src)?.clone();
                if matches!(self, Op::MigrateOs { .. }) && !(s.is_system || s.is_boot) {
                    return Err(crate::tr!("Le disque source doit être celui où Windows est installé.", "The source disk must be the one Windows is installed on.").into());
                }
                if s.style == Style::Raw || s.parts.is_empty() {
                    return Err(crate::tr!("Le disque source est vide.", "The source disk is empty.").into());
                }
                let d = disk_mut(disks, *dst)?;
                if d.is_system || d.is_boot {
                    return Err(crate::tr!("Le disque de Windows ne peut pas servir de cible.", "The Windows disk cannot be the target.").into());
                }
                if d.sector != s.sector {
                    return Err(crate::tr!("Les deux disques n'ont pas la même taille de secteur : copie secteur par secteur impossible.", "The two disks have different sector sizes: a sector-by-sector copy is not possible.").into());
                }
                let need = s.used_extent() + if s.style == Style::Gpt { gpt_tail_sectors(s.sector) * s.sector } else { 0 };
                if d.size < need {
                    return Err(crate::trf!("Le disque cible est trop petit : il faut au moins {}. Réduisez d'abord la dernière partition du disque source.", "The target disk is too small: it needs at least {}. Shrink the source disk's last partition first.",
                        fmt_size(need)
                    ));
                }
                let size = d.size;
                *d = Disk { number: d.number, model: d.model.clone(), size, bus: d.bus.clone(), uid: d.uid.clone(), is_system: false, is_boot: false, offline: false, pending: true, ..s.clone() };
                for p in d.parts.iter_mut() {
                    p.letter = None;
                    p.is_boot = false;
                    p.is_system = false;
                    p.pending = true;
                }
                if *fit {
                    let end = d.usable_end();
                    if let Some(last) = d.parts.iter_mut().max_by_key(|p| p.offset) {
                        if last.kind == Kind::Data && last.fs == Fs::Ntfs {
                            last.size = align_down(end - last.offset, MIB);
                        }
                    }
                }
            }
            Op::ToGpt { disk } => {
                let d = disk_mut(disks, *disk)?;
                match d.style {
                    Style::Gpt => return Err(crate::tr!("Ce disque est déjà en GPT.", "This disk is already GPT.").into()),
                    Style::Raw => {}
                    Style::Mbr => {
                        if d.dynamic {
                            return Err(crate::tr!("Convertissez d'abord le disque dynamique en disque de base.", "Convert the dynamic disk to a basic disk first.").into());
                        }
                        let head = gpt_head_sectors(d.sector) * d.sector;
                        let tail = gpt_tail_sectors(d.sector) * d.sector;
                        let first = d.parts.iter().filter(|p| p.kind != Kind::Extended).map(|p| p.offset).min().unwrap_or(head);
                        if first < head {
                            return Err(crate::tr!("La conversion est impossible : la première partition commence trop tôt. Redimensionnez-la puis réessayez.", "The conversion is not possible: the first partition starts too early. Resize it, then try again.").into());
                        }
                        if d.used_extent() > d.size - tail {
                            return Err(crate::tr!("La conversion est impossible : il faut un peu d'espace libre à la fin du disque. Réduisez la dernière partition puis réessayez.", "The conversion is not possible: a little free space is needed at the end of the disk. Shrink the last partition, then try again.").into());
                        }
                        if d.parts.iter().filter(|p| p.kind != Kind::Extended).count() > 128 {
                            return Err(crate::tr!("Trop de partitions pour GPT.", "Too many partitions for GPT.").into());
                        }
                    }
                }
                d.style = Style::Gpt;
                d.parts.retain(|p| p.kind != Kind::Extended);
                for p in d.parts.iter_mut() {
                    if p.kind == Kind::Logical {
                        p.kind = Kind::Data;
                    }
                    p.is_active = false;
                }
                d.pending = true;
            }
            Op::ToMbr { disk } => {
                let d = disk_mut(disks, *disk)?;
                match d.style {
                    Style::Mbr => return Err(crate::tr!("Ce disque est déjà en MBR.", "This disk is already MBR.").into()),
                    Style::Raw => {}
                    Style::Gpt => {
                        if d.is_system || d.is_boot {
                            return Err(crate::tr!("Le disque de Windows démarre en UEFI et doit rester en GPT.", "The Windows disk starts in UEFI mode and must stay GPT.").into());
                        }
                        if d.dynamic {
                            return Err(crate::tr!("Convertissez d'abord le disque dynamique en disque de base.", "Convert the dynamic disk to a basic disk first.").into());
                        }
                        let keep: Vec<&Partition> = d.parts.iter().filter(|p| p.kind != Kind::Msr).collect();
                        if keep.len() > 4 {
                            return Err(crate::tr!("Un disque MBR accepte au plus 4 partitions principales. Supprimez ou fusionnez des partitions puis réessayez.", "An MBR disk accepts at most 4 primary partitions. Delete or merge partitions, then try again.").into());
                        }
                        let max = mbr_max_bytes(d.sector);
                        if keep.iter().any(|p| p.end() > max) {
                            return Err(crate::trf!("Le disque GPT sélectionné ne peut pas être converti en MBR, car il contient une partition qui dépasse {}.", "The selected GPT disk cannot be converted to MBR, because it contains a partition that goes beyond {}.",
                                fmt_size(max)
                            ));
                        }
                    }
                }
                d.style = Style::Mbr;
                d.parts.retain(|p| p.kind != Kind::Msr);
                d.pending = true;
            }
            Op::NtfsToFat32 { disk, offset, temp } => {
                let tfree = disks
                    .iter()
                    .flat_map(|d| d.parts.iter())
                    .find(|p| p.letter == Some(*temp))
                    .and_then(|p| p.free())
                    .ok_or_else(|| crate::trf!("Le lecteur {temp}: est introuvable.", "Drive {temp}: cannot be found."))?;
                let d = disk_mut(disks, *disk)?;
                let i = part_of(d, *offset)?;
                let p = &mut d.parts[i];
                if p.fs != Fs::Ntfs {
                    return Err(crate::tr!("La partition doit être en NTFS.", "The partition must be NTFS.").into());
                }
                if p.protected() {
                    return Err(crate::tr!("La partition de Windows ne peut pas être convertie en FAT32.", "The Windows partition cannot be converted to FAT32.").into());
                }
                if p.letter.is_none() {
                    return Err(crate::tr!("Donnez d'abord une lettre à la partition.", "Give the partition a letter first.").into());
                }
                if p.letter == Some(*temp) {
                    return Err(crate::tr!("Choisissez un autre lecteur pour la copie temporaire.", "Choose another drive for the temporary copy.").into());
                }
                if p.size > 2 * TIB {
                    return Err(crate::tr!("FAT32 est limité à 2 To.", "FAT32 is limited to 2 TB.").into());
                }
                let used = p.used.unwrap_or(p.size);
                if used + 100 * MIB > tfree {
                    return Err(crate::trf!("Pas assez d'espace libre sur {temp}: pour la copie temporaire ({} nécessaires).", "Not enough free space on {temp}: for the temporary copy ({} needed).", fmt_size(used + 100 * MIB)));
                }
                p.fs = Fs::Fat32;
                p.label = p.label.to_uppercase().chars().take(11).collect();
                p.pending = true;
            }
            Op::DynamicToBasic { disk, temp } => {
                let tfree = disks
                    .iter()
                    .flat_map(|d| d.parts.iter())
                    .find(|p| p.letter == Some(*temp))
                    .and_then(|p| p.free())
                    .ok_or_else(|| crate::trf!("Le lecteur {temp}: est introuvable.", "Drive {temp}: cannot be found."))?;
                let d = disk_mut(disks, *disk)?;
                if !d.dynamic {
                    return Err(crate::tr!("Ce disque est déjà un disque de base.", "This disk is already a basic disk.").into());
                }
                if d.is_system || d.is_boot {
                    return Err(crate::tr!("Le disque de Windows ne peut pas être converti pendant qu'il fonctionne.", "The Windows disk cannot be converted while it is running.").into());
                }
                if d.parts.iter().any(|p| p.letter == Some(*temp)) {
                    return Err(crate::tr!("La copie temporaire doit aller sur un autre disque.", "The temporary copy must go to another disk.").into());
                }
                let used: u64 = d.parts.iter().filter_map(|p| p.used).sum();
                if used + 100 * MIB > tfree {
                    return Err(crate::trf!("Pas assez d'espace libre sur {temp}: pour la copie temporaire.", "Not enough free space on {temp}: for the temporary copy."));
                }
                d.dynamic = false;
                d.parts.retain(|p| p.kind != Kind::LdmMeta);
                for p in d.parts.iter_mut() {
                    if p.kind == Kind::LdmData {
                        p.kind = Kind::Data;
                    }
                    p.pending = true;
                }
            }
        }
        Ok(())
    }

    /// Worker steps for this operation. `before` is the preview just before it.
    pub fn steps(&self, before: &[Disk]) -> Result<Vec<Step>, String> {
        let pn = |d: u32, o: u64| format!("(P {d} {o}).PartitionNumber");
        let mut out = Vec::new();
        match self {
            Op::Resize { disk, offset, new_offset, new_size } => {
                let d = disk_ref(before, *disk)?;
                let p = &d.parts[part_of(d, *offset)?];
                let resize = |size: u64, at: u64| Step::Ps {
                    script: format!("# Redimensionner la partition\nResize-Partition -DiskNumber {disk} -PartitionNumber {} -Size {size}", pn(*disk, at)),
                };
                // Shrink first (less data to move), grow last (into the space freed by the move).
                let copy_size = (*new_size).min(p.size);
                if *new_size < p.size {
                    out.push(resize(*new_size, *offset));
                }
                if new_offset != offset {
                    if p.kind == Kind::Recovery {
                        out.push(Step::Ps { script: "# Mettre en pause l'environnement de récupération\nreagentc /disable | Out-Null\n$global:LASTEXITCODE = 0".into() });
                    }
                    out.push(Step::MovePart { disk: *disk, offset: *offset, size: copy_size, new_offset: *new_offset, sector: d.sector });
                    let mut s = format!("# Remonter la partition déplacée\nUpdate-Disk -Number {disk}\nStart-Sleep -Seconds 2\n$p = P {disk} {new_offset}");
                    if let Some(l) = p.letter {
                        s += &format!(
                            "\nif (-not $p.DriveLetter -or [string]$p.DriveLetter -eq [string][char]0) {{ Add-PartitionAccessPath -DiskNumber {disk} -PartitionNumber $p.PartitionNumber -AccessPath '{l}:\\' -ErrorAction SilentlyContinue }}"
                        );
                    }
                    if p.kind == Kind::Recovery {
                        s += "\nreagentc /enable | Out-Null\n$global:LASTEXITCODE = 0";
                    }
                    out.push(Step::Ps { script: s });
                }
                if *new_size > p.size {
                    out.push(resize(*new_size, *new_offset));
                }
            }
            Op::Create { disk, offset, size, fs, label, letter } => {
                let l = match letter {
                    Some(c) => format!("-DriveLetter {c}"),
                    None => "-AssignDriveLetter".into(),
                };
                out.push(Step::Ps { script: format!("# Créer une partition\n$p = New-Partition -DiskNumber {disk} -Offset {offset} -Size {size} {l}") });
                out.extend(format_steps(before, *disk, *offset, *size, fs, label, 0, true));
            }
            Op::Format { disk, offset, fs, label, cluster, quick } => {
                let d = disk_ref(before, *disk)?;
                let p = &d.parts[part_of(d, *offset)?];
                out.extend(format_steps(before, *disk, *offset, p.size, fs, label, *cluster, *quick));
            }
            Op::Delete { disk, offset } => out.push(Step::Ps {
                script: format!("# Supprimer la partition\nRemove-Partition -DiskNumber {disk} -PartitionNumber {} -Confirm:$false", pn(*disk, *offset)),
            }),
            Op::WipePart { disk, offset, passes } => out.push(Step::WipePart { disk: *disk, offset: *offset, passes: *passes }),
            Op::WipeDisk { disk, passes } => {
                out.push(Step::Ps { script: format!("# Préparer le disque {disk}\nClear-Disk -Number {disk} -RemoveData -RemoveOEM -Confirm:$false -ErrorAction SilentlyContinue\nSet-Disk -Number {disk} -IsOffline $true -ErrorAction SilentlyContinue") });
                out.push(Step::WipeDisk { disk: *disk, passes: *passes });
                out.push(Step::Ps { script: format!("# Remettre le disque {disk} en ligne\nSet-Disk -Number {disk} -IsOffline $false -ErrorAction SilentlyContinue\nUpdate-Disk -Number {disk}") });
            }
            Op::Merge { disk, first } => {
                let d = disk_ref(before, *disk)?;
                let a = &d.parts[part_of(d, *first)?];
                let b = &d.parts[d.next_part(*first).ok_or(crate::tr!("partition suivante", "next partition"))?];
                let al = a.letter.ok_or(crate::tr!("lettre", "drive letter"))?;
                let mut s = String::from("# Fusionner deux partitions\n");
                if let Some(bl) = b.letter {
                    s += &merge_copy_script(al, bl);
                }
                s += &format!("Remove-Partition -DiskNumber {disk} -PartitionNumber {} -Confirm:$false\n", pn(*disk, b.offset));
                s += &format!("Resize-Partition -DiskNumber {disk} -PartitionNumber {} -Size {}", pn(*disk, a.offset), b.end() - a.offset);
                out.push(Step::Ps { script: s });
            }
            Op::SetLetter { disk, offset, letter } => {
                let d = disk_ref(before, *disk)?;
                let p = &d.parts[part_of(d, *offset)?];
                let mut s = String::from("# Changer la lettre de lecteur\n");
                if let Some(old) = p.letter {
                    s += &format!("Remove-PartitionAccessPath -DiskNumber {disk} -PartitionNumber {} -AccessPath '{old}:\\'\n", pn(*disk, *offset));
                }
                if let Some(l) = letter {
                    s += &format!("Add-PartitionAccessPath -DiskNumber {disk} -PartitionNumber {} -AccessPath '{l}:\\'", pn(*disk, *offset));
                }
                out.push(Step::Ps { script: s });
            }
            Op::SetLabel { disk, offset, label } => out.push(Step::Ps {
                script: format!("# Changer le nom de la partition\nP {disk} {offset} | Get-Volume | Set-Volume -NewFileSystemLabel {}", ps_quote(label)),
            }),
            Op::SetActive { disk, offset } => out.push(Step::Ps {
                script: format!("# Rendre la partition active\nSet-Partition -DiskNumber {disk} -PartitionNumber {} -IsActive $true", pn(*disk, *offset)),
            }),
            Op::ClonePart { src_disk, src_offset, dst_disk, dst_offset } => {
                let s = disk_ref(before, *src_disk)?;
                let p = &s.parts[part_of(s, *src_offset)?];
                out.push(Step::Ps { script: format!("# Préparer la place de la copie\nNew-Partition -DiskNumber {dst_disk} -Offset {dst_offset} -Size {} | Out-Null", p.size) });
                out.push(Step::ClonePart {
                    src_disk: *src_disk,
                    part: SrcPart { offset: p.offset, size: p.size, letter: p.letter, ntfs: p.fs == Fs::Ntfs },
                    dst_disk: *dst_disk,
                    dst_offset: *dst_offset,
                });
                out.push(Step::Ps { script: format!("# Donner une lettre à la copie\nAdd-PartitionAccessPath -DiskNumber {dst_disk} -PartitionNumber {} -AssignDriveLetter -ErrorAction SilentlyContinue", pn(*dst_disk, *dst_offset)) });
            }
            Op::CloneDisk { src, dst, fit } | Op::MigrateOs { src, dst, fit } => {
                let s = disk_ref(before, *src)?;
                out.push(Step::Ps {
                    script: format!(
                        "# Préparer le disque cible {dst}\nSet-Disk -Number {dst} -IsOffline $false -ErrorAction SilentlyContinue\nSet-Disk -Number {dst} -IsReadOnly $false -ErrorAction SilentlyContinue\nClear-Disk -Number {dst} -RemoveData -RemoveOEM -Confirm:$false -ErrorAction SilentlyContinue\nSet-Disk -Number {dst} -IsOffline $true -ErrorAction SilentlyContinue"
                    ),
                });
                out.push(Step::CloneDisk {
                    src: *src,
                    dst: *dst,
                    sector: s.sector,
                    parts: s
                        .parts
                        .iter()
                        .filter(|p| p.kind != Kind::Extended)
                        .map(|p| SrcPart { offset: p.offset, size: p.size, letter: p.letter, ntfs: p.fs == Fs::Ntfs })
                        .collect(),
                    extent: s.used_extent(),
                });
                out.push(Step::FixIds { disk: *dst, sector: s.sector });
                out.push(Step::Ps { script: format!("# Remettre le disque {dst} en ligne\nSet-Disk -Number {dst} -IsOffline $false\nUpdate-Disk -Number {dst}") });
                if *fit {
                    if let Some(last) = s.parts.iter().filter(|p| p.kind != Kind::Extended).max_by_key(|p| p.offset) {
                        if last.kind == Kind::Data && last.fs == Fs::Ntfs {
                            out.push(Step::Ps {
                                script: format!(
                                    "# Agrandir la dernière partition au disque cible\n$n = {}\n$m = (Get-PartitionSupportedSize -DiskNumber {dst} -PartitionNumber $n).SizeMax\nResize-Partition -DiskNumber {dst} -PartitionNumber $n -Size $m",
                                    pn(*dst, last.offset)
                                ),
                            });
                        }
                    }
                }
                if let Op::MigrateOs { .. } = self {
                    let win = s.parts.iter().find(|p| p.is_boot).or_else(|| s.parts.iter().find(|p| p.letter == Some('C'))).ok_or(crate::tr!("partition de Windows introuvable", "Windows partition not found"))?;
                    let sys = if s.style == Style::Gpt {
                        s.parts.iter().find(|p| p.kind == Kind::Efi)
                    } else {
                        s.parts.iter().find(|p| p.is_active).or(Some(win))
                    }
                    .ok_or(crate::tr!("partition de démarrage introuvable", "boot partition not found"))?;
                    out.push(Step::FixBoot { disk: *dst, win_offset: win.offset, sys_offset: sys.offset, uefi: s.style == Style::Gpt });
                }
            }
            Op::ToGpt { disk } => {
                let d = disk_ref(before, *disk)?;
                match d.style {
                    Style::Raw => out.push(Step::Ps { script: format!("# Initialiser le disque {disk} en GPT\nInitialize-Disk -Number {disk} -PartitionStyle GPT") }),
                    _ if d.is_system || d.is_boot => out.push(Step::Ps {
                        script: format!("# Convertir le disque de Windows en GPT\nmbr2gpt.exe /validate /disk:{disk} /allowFullOS\nif ($LASTEXITCODE -ne 0) {{ throw 'Le disque ne remplit pas les conditions de mbr2gpt.' }}\nmbr2gpt.exe /convert /disk:{disk} /allowFullOS\nif ($LASTEXITCODE -ne 0) {{ throw 'mbr2gpt a échoué.' }}\nWrite-Output 'Pensez à passer le BIOS du PC en mode UEFI avant de redémarrer.'"),
                    }),
                    _ => {
                        out.push(Step::ToGpt { disk: *disk, sector: d.sector });
                        out.push(Step::Ps { script: format!("# Relire le disque {disk}\nUpdate-Disk -Number {disk}") });
                    }
                }
            }
            Op::ToMbr { disk } => {
                let d = disk_ref(before, *disk)?;
                if d.style == Style::Raw {
                    out.push(Step::Ps { script: format!("# Initialiser le disque {disk} en MBR\nInitialize-Disk -Number {disk} -PartitionStyle MBR") });
                } else {
                    let fat32 = d.parts.iter().filter(|p| p.fs == Fs::Fat32).map(|p| p.offset).collect();
                    out.push(Step::ToMbr { disk: *disk, sector: d.sector, fat32_offsets: fat32 });
                    out.push(Step::Ps { script: format!("# Relire le disque {disk}\nUpdate-Disk -Number {disk}") });
                }
            }
            Op::NtfsToFat32 { disk, offset, temp } => {
                let d = disk_ref(before, *disk)?;
                let p = &d.parts[part_of(d, *offset)?];
                let l = p.letter.ok_or(crate::tr!("lettre", "drive letter"))?;
                let tmp = ps_quote(&format!("{temp}:\\DiskCraft-FAT32-{l}"));
                out.push(Step::Ps {
                    script: format!(
                        "# Vérifier et copier les fichiers de {l}:\n$big = Get-ChildItem {l}:\\ -Recurse -File -Force -ErrorAction SilentlyContinue | Where-Object Length -gt 4294967295 | Select-Object -First 1\nif ($big) {{ throw \"FAT32 n'accepte pas les fichiers de plus de 4 Go : $($big.FullName)\" }}\nrobocopy {l}:\\ {tmp} /E /COPY:DAT /XJ /R:1 /W:1 /NP /XD 'System Volume Information' '$RECYCLE.BIN'\nif ($LASTEXITCODE -ge 8) {{ throw 'La copie temporaire a échoué.' }}"
                    ),
                });
                let label: String = p.label.to_uppercase().chars().take(11).collect();
                out.push(Step::FormatFat32 { disk: *disk, offset: *offset, label, cluster: 0 });
                let mut back = format!(
                    "# Remettre les fichiers sur {l}:\nStart-Sleep -Seconds 2\nrobocopy {tmp} {l}:\\ /E /COPY:DT /R:1 /W:1 /NP\nif ($LASTEXITCODE -ge 8) {{ throw \"La remise en place a échoué : vos fichiers sont toujours dans {temp}:\\DiskCraft-FAT32-{l}\" }}\nRemove-Item {tmp} -Recurse -Force"
                );
                if d.style == Style::Mbr {
                    back += &format!("\nSet-Partition -DiskNumber {disk} -PartitionNumber {} -MbrType 12 -ErrorAction SilentlyContinue", pn(*disk, *offset));
                }
                out.push(Step::Ps { script: back });
            }
            Op::DynamicToBasic { disk, temp } => {
                out.push(Step::Ps { script: dynamic_to_basic_script(*disk, *temp) });
            }
        }
        Ok(out)
    }
}

impl Disk {
    pub fn offset_end_ok(&self, end: u64) -> Result<(), ()> {
        if end > mbr_max_bytes(self.sector) {
            Err(())
        } else {
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn format_steps(before: &[Disk], disk: u32, offset: u64, size: u64, fs: &Fs, label: &str, cluster: u32, quick: bool) -> Vec<Step> {
    let _ = before;
    if *fs == Fs::Fat32 && size > 32 * GIB {
        // Windows' own tools stop at 32 GB for FAT32: DiskCraft writes it itself.
        return vec![Step::FormatFat32 { disk, offset, label: label.to_uppercase().chars().take(11).collect(), cluster }];
    }
    let mut s = format!(
        "# Formater en {}\nFormat-Volume -Partition (P {disk} {offset}) -FileSystem {} -NewFileSystemLabel {} -Force -Confirm:$false",
        fs.name(),
        fs.ps_name(),
        ps_quote(label)
    );
    if cluster > 0 {
        s += &format!(" -AllocationUnitSize {cluster}");
    }
    if !quick {
        s += " -Full";
    }
    s += " | Out-Null";
    vec![Step::Ps { script: s }]
}

/// Copies every file of `src:` to the root of `dst:` before the merge deletes `src:`.
/// Files whose name is already used on `dst:` are copied as `name-HH-mm-ss-dd-MM-yyyy.ext`.
/// Nothing is deleted unless every file is found on `dst:` with its size.
fn merge_copy_script(dst: char, src: char) -> String {
    let msgs = [
        ("__M_COPY_FAILED__", crate::tr!("La copie du contenu a échoué : rien n'a été supprimé.", "Copying the contents failed: nothing was deleted.")),
        ("__M_RENAMED__", crate::tr!("Renommé (nom déjà utilisé) : ", "Renamed (name already used): ")),
        ("__M_MISSING__", crate::tr!("Manquant : ", "Missing: ")),
        ("__M_NOT_COPIED__", crate::tr!(" fichier(s) n'ont pas été copiés : la seconde partition n'est pas supprimée.", " file(s) were not copied: the second partition is not deleted.")),
        ("__M_FOLDER_CLASH__", crate::tr!("Un fichier porte le nom d'un dossier à copier : ", "A file has the name of a folder to copy: ")),
        ("__M_DONE__", crate::tr!(" fichier(s) copiés, dont renommés : ", " file(s) copied, renamed: ")),
    ];
    let mut s = r#"$src = '__SRC__:\'
$dst = '__DST__:\'
$stamp = Get-Date -Format 'HH-mm-ss-dd-MM-yyyy'
$skip = @('System Volume Information', '$RECYCLE.BIN')
$files = New-Object System.Collections.Generic.List[object]
$xd = New-Object System.Collections.Generic.List[string]
foreach ($s in $skip) { $xd.Add((Join-Path $src $s)) }
# 1. Files of the second partition (Windows folders and links left out, as robocopy does).
function Add-Files([string]$dir, [string]$rel) {
  foreach ($e in @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop)) {
    $r = if ($rel) { $rel + '\' + $e.Name } else { $e.Name }
    if ($e.PSIsContainer) {
      if (-not $rel -and $skip -contains $e.Name) { continue }
      if ($e.Attributes -band [IO.FileAttributes]::ReparsePoint) { $xd.Add($e.FullName); continue }
      Add-Files $e.FullName $r
    } else {
      $files.Add([pscustomobject]@{ Rel = $r; Size = $e.Length; Full = $e.FullName; Dest = (Join-Path $dst $r) })
    }
  }
}
Add-Files $src ''
# ...and the ones whose name is already used on the first partition.
$clash = @($files | Where-Object { Test-Path -LiteralPath $_.Dest })
# 2. Everything else: robocopy leaves out every file that already exists on the first partition.
robocopy $src $dst /E /COPY:DAT /DCOPY:T /XJ /R:1 /W:1 /NP /NFL /NDL /XC /XN /XO /XD $xd.ToArray() | Out-Null
if ($LASTEXITCODE -ge 8) { throw (__M_COPY_FAILED__) }
# 3. Same names: the copy gets the date and time in its name.
foreach ($f in $clash) {
  $dir = Split-Path -Parent $f.Dest
  if (-not (Test-Path -LiteralPath $dir -PathType Container)) { throw ((__M_FOLDER_CLASH__) + $f.Rel) }
  $base = [IO.Path]::GetFileNameWithoutExtension($f.Dest)
  $ext = [IO.Path]::GetExtension($f.Dest)
  $new = Join-Path $dir ($base + '-' + $stamp + $ext)
  $n = 1
  while (Test-Path -LiteralPath $new) { $new = Join-Path $dir ($base + '-' + $stamp + '-' + $n + $ext); $n++ }
  Copy-Item -LiteralPath $f.Full -Destination $new -Force
  $f.Dest = $new
  Write-Output ((__M_RENAMED__) + $f.Rel + ' -> ' + (Split-Path -Leaf $new))
}
# 4. Every file must be on the first partition, with its size, before anything is deleted.
$missing = 0
foreach ($f in $files) {
  $i = Get-Item -LiteralPath $f.Dest -Force -ErrorAction SilentlyContinue
  if (-not $i -or $i.PSIsContainer -or $i.Length -ne $f.Size) {
    $missing++
    if ($missing -le 20) { Write-Output ((__M_MISSING__) + $f.Rel) }
  }
}
if ($missing -gt 0) { throw ([string]$missing + (__M_NOT_COPIED__)) }
Write-Output ([string]$files.Count + (__M_DONE__) + [string]$clash.Count)
"#
    .replace("__SRC__", &src.to_string())
    .replace("__DST__", &dst.to_string());
    for (k, v) in msgs {
        s = s.replace(k, &ps_quote(v));
    }
    s
}

fn dynamic_to_basic_script(disk: u32, temp: char) -> String {
    format!(
        r#"# Convertir le disque dynamique {disk} en disque de base
$dp = Join-Path $env:TEMP 'diskcraft_dp.txt'
"select disk {disk}`ndetail disk" | Set-Content -Encoding ASCII $dp
$detail = diskpart /s $dp
$vols = @()
foreach ($line in $detail) {{
  if ($line -match '^\s*\S+\s+\d+\s+([A-Z])\s') {{
    $v = Get-Volume -DriveLetter $Matches[1]
    if ($v) {{ $vols += [pscustomobject]@{{Letter=$Matches[1];Fs=[string]$v.FileSystem;Label=[string]$v.FileSystemLabel;Size=[uint64]$v.Size}} }}
  }}
}}
if ($vols.Count -eq 0) {{ throw 'Aucun volume avec une lettre n a été trouvé sur ce disque.' }}
foreach ($v in $vols) {{
  $t = '{temp}:\DiskCraft-Dyn-' + $v.Letter
  Write-Output "Copie de $($v.Letter): vers $t"
  robocopy "$($v.Letter):\" $t /E /COPY:DAT /XJ /R:1 /W:1 /NP /XD 'System Volume Information' '$RECYCLE.BIN' | Out-Null
  if ($LASTEXITCODE -ge 8) {{ throw "La copie de $($v.Letter): a échoué." }}
}}
$s = "select disk {disk}`n"
foreach ($v in $vols) {{ $s += "select volume $($v.Letter)`ndelete volume override`n" }}
$s += "select disk {disk}`nconvert basic noerr`n"
for ($i = 0; $i -lt $vols.Count; $i++) {{
  $v = $vols[$i]
  $fs = if ($v.Fs -in @('NTFS','FAT32','exFAT')) {{ $v.Fs }} else {{ 'NTFS' }}
  if ($i -lt $vols.Count - 1) {{ $s += "create partition primary size=$([math]::Floor($v.Size / 1MB))`n" }} else {{ $s += "create partition primary`n" }}
  $s += "format fs=$fs quick label=`"$($v.Label)`"`nassign letter=$($v.Letter)`n"
}}
$s | Set-Content -Encoding ASCII $dp
diskpart /s $dp
if ($LASTEXITCODE -ne 0) {{ throw "diskpart a échoué : vos fichiers sont dans {temp}:\DiskCraft-Dyn-*" }}
Start-Sleep -Seconds 3
foreach ($v in $vols) {{
  $t = '{temp}:\DiskCraft-Dyn-' + $v.Letter
  robocopy $t "$($v.Letter):\" /E /COPY:DAT /R:1 /W:1 /NP | Out-Null
  if ($LASTEXITCODE -ge 8) {{ throw "La remise en place de $($v.Letter): a échoué : les fichiers restent dans $t" }}
  Remove-Item $t -Recurse -Force
}}
Remove-Item $dp -ErrorAction SilentlyContinue"#
    )
}

/// One-step plans that run at once (they change nothing in the layout).
pub fn check_steps(d: &Disk, p: &Partition, mode: usize) -> Vec<Step> {
    let target = match p.letter {
        Some(l) => format!("-DriveLetter {l}"),
        None => format!("-Path (P {} {} | Get-Volume).Path", d.number, p.offset),
    };
    match mode {
        0 => vec![Step::Ps { script: format!("# Rechercher les erreurs\nRepair-Volume {target} -Scan") }],
        1 => vec![Step::Ps { script: format!("# Vérifier et réparer les erreurs\nRepair-Volume {target} -OfflineScanAndFix") }],
        _ => vec![Step::SurfaceTest { disk: d.number, offset: p.offset, size: p.size, sector: d.sector }],
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::merge_copy_script;

    /// Runs the merge copy between two temporary folders: files go to the root of the first one,
    /// a file with a name already used gets the date and time in its name.
    #[test]
    fn merge_copy_renames_same_names() {
        let root = std::env::temp_dir().join(format!("diskcraft-merge-test-{}", std::process::id()));
        let (src, dst) = (root.join("src"), root.join("dst"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(dst.join("sub")).unwrap();
        std::fs::write(src.join("test.txt"), "from second partition").unwrap();
        std::fs::write(src.join("only-src.txt"), "x").unwrap();
        std::fs::write(src.join("sub").join("deep.txt"), "deep").unwrap();
        std::fs::write(dst.join("test.txt"), "already on first partition").unwrap();

        let script = merge_copy_script('Y', 'Z')
            .replace(r"$src = 'Z:\'", &format!(r"$src = '{}\'", src.display()))
            .replace(r"$dst = 'Y:\'", &format!(r"$dst = '{}\'", dst.display()));
        let mut out = Vec::new();
        let r = crate::worker::run_ps_file(&script, &mut |l| out.push(l.to_string()));
        assert!(r.is_ok(), "script failed: {r:?}\n{}", out.join("\n"));

        assert_eq!(std::fs::read_to_string(dst.join("test.txt")).unwrap(), "already on first partition");
        assert!(dst.join("only-src.txt").exists());
        assert!(dst.join("sub").join("deep.txt").exists());
        let renamed: Vec<_> = std::fs::read_dir(&dst)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("test-") && n.ends_with(".txt"))
            .collect();
        assert_eq!(renamed.len(), 1, "files: {renamed:?}");
        // test-HH-mm-ss-dd-MM-yyyy.txt
        assert_eq!(renamed[0].len(), "test-11-23-25-20-11-2026.txt".len());
        assert_eq!(std::fs::read_to_string(dst.join(&renamed[0])).unwrap(), "from second partition");
        let _ = std::fs::remove_dir_all(&root);
    }
}
