//! Partition tables: reading and writing MBR and GPT, written from the public formats.

use crate::model::*;
use crate::rawio::{seed, Dev, XorShift};

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// GUID text to its on-disk bytes (first three fields little-endian).
pub fn guid_bytes(s: &str) -> [u8; 16] {
    let h: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let mut raw = [0u8; 16];
    for i in 0..16 {
        raw[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    let mut out = raw;
    out[0..4].copy_from_slice(&[raw[3], raw[2], raw[1], raw[0]]);
    out[4..6].copy_from_slice(&[raw[5], raw[4]]);
    out[6..8].copy_from_slice(&[raw[7], raw[6]]);
    out
}

pub fn new_guid(rng: &mut XorShift) -> [u8; 16] {
    let mut g = [0u8; 16];
    rng.fill(&mut g);
    g[7] = (g[7] & 0x0F) | 0x40; // version 4 (stored little-endian)
    g[8] = (g[8] & 0x3F) | 0x80;
    g
}

const GUID_LINUX: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";
const GUID_LINUX_SWAP: &str = "0657FD6D-A4AB-43C4-84E5-0933C84B4F4F";
const ENTRY_COUNT: u64 = 128;
const ENTRY_SIZE: u64 = 128;

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

struct MbrPart {
    ptype: u8,
    start: u64,
    count: u64,
}

fn read_mbr_parts(dev: &mut Dev, ss: u64) -> Result<(Vec<u8>, Vec<MbrPart>), String> {
    let mut s0 = vec![0u8; ss as usize];
    dev.read_at(0, &mut s0)?;
    if s0[510] != 0x55 || s0[511] != 0xAA {
        return Err("La table de partitions MBR est illisible.".into());
    }
    let mut parts = Vec::new();
    for i in 0..4 {
        let e = 446 + i * 16;
        let ptype = s0[e + 4];
        let start = u32le(&s0, e + 8) as u64;
        let count = u32le(&s0, e + 12) as u64;
        if ptype == 0 || count == 0 {
            continue;
        }
        if matches!(ptype, 0x05 | 0x0F | 0x85) {
            // Walk the chain of logical partitions.
            let base = start;
            let mut cur = start;
            for _ in 0..128 {
                let mut eb = vec![0u8; ss as usize];
                dev.read_at(cur * ss, &mut eb)?;
                if eb[510] != 0x55 || eb[511] != 0xAA {
                    break;
                }
                let t = eb[446 + 4];
                let rel = u32le(&eb, 446 + 8) as u64;
                let cnt = u32le(&eb, 446 + 12) as u64;
                if t != 0 && cnt != 0 {
                    parts.push(MbrPart { ptype: t, start: cur + rel, count: cnt });
                }
                let nt = eb[462 + 4];
                let nrel = u32le(&eb, 462 + 8) as u64;
                if nt == 0 || nrel == 0 {
                    break;
                }
                cur = base + nrel;
            }
        } else {
            parts.push(MbrPart { ptype, start, count });
        }
    }
    parts.sort_by_key(|p| p.start);
    Ok((s0, parts))
}

fn entries_sectors(ss: u64) -> u64 {
    (ENTRY_COUNT * ENTRY_SIZE).div_ceil(ss)
}

/// Write primary and backup GPT around `entries` (128 x 128 bytes) on a disk of `total` sectors.
fn write_gpt(dev: &mut Dev, ss: u64, total: u64, disk_guid: [u8; 16], entries: &[u8]) -> Result<(), String> {
    let es = entries_sectors(ss);
    let first_usable = 2 + es;
    let last_usable = total - 2 - es;
    let ecrc = crc32(entries);
    let header = |my: u64, alt: u64, elba: u64| -> Vec<u8> {
        let mut h = vec![0u8; ss as usize];
        h[0..8].copy_from_slice(b"EFI PART");
        h[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        h[12..16].copy_from_slice(&92u32.to_le_bytes());
        h[24..32].copy_from_slice(&my.to_le_bytes());
        h[32..40].copy_from_slice(&alt.to_le_bytes());
        h[40..48].copy_from_slice(&first_usable.to_le_bytes());
        h[48..56].copy_from_slice(&last_usable.to_le_bytes());
        h[56..72].copy_from_slice(&disk_guid);
        h[72..80].copy_from_slice(&elba.to_le_bytes());
        h[80..84].copy_from_slice(&(ENTRY_COUNT as u32).to_le_bytes());
        h[84..88].copy_from_slice(&(ENTRY_SIZE as u32).to_le_bytes());
        h[88..92].copy_from_slice(&ecrc.to_le_bytes());
        let c = crc32(&h[0..92]);
        h[16..20].copy_from_slice(&c.to_le_bytes());
        h
    };
    let mut ebuf = entries.to_vec();
    ebuf.resize((es * ss) as usize, 0);
    // Backup first, primary last: the disk stays readable if something stops half way.
    dev.write_at((total - 1 - es) * ss, &ebuf)?;
    dev.write_at((total - 1) * ss, &header(total - 1, 1, total - 1 - es))?;
    dev.write_at(2 * ss, &ebuf)?;
    dev.write_at(ss, &header(1, total - 1, 2))?;
    Ok(())
}

fn protective_mbr(old: &[u8], total: u64) -> Vec<u8> {
    let mut m = old.to_vec();
    for b in m[446..510].iter_mut() {
        *b = 0;
    }
    let e = 446;
    m[e + 1..e + 4].copy_from_slice(&[0x00, 0x02, 0x00]);
    m[e + 4] = 0xEE;
    m[e + 5..e + 8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    m[e + 8..e + 12].copy_from_slice(&1u32.to_le_bytes());
    m[e + 12..e + 16].copy_from_slice(&((total - 1).min(0xFFFF_FFFF) as u32).to_le_bytes());
    m[510] = 0x55;
    m[511] = 0xAA;
    m
}

fn utf16_name(name: &str) -> Vec<u8> {
    name.encode_utf16().take(36).flat_map(|c| c.to_le_bytes()).collect()
}

/// MBR to GPT without touching the data.
pub fn mbr_to_gpt(disk: u32, ss: u64) -> Result<(), String> {
    let mut dev = Dev::disk(disk, true)?;
    let total = dev.len()? / ss;
    let (s0, parts) = read_mbr_parts(&mut dev, ss)?;
    let es = entries_sectors(ss);
    if parts.iter().any(|p| p.start < 2 + es) {
        return Err("La première partition commence trop tôt pour GPT.".into());
    }
    if parts.iter().any(|p| p.start + p.count > total - 2 - es) {
        return Err("Il faut un peu d'espace libre à la fin du disque pour la copie de secours GPT.".into());
    }
    if parts.iter().any(|p| p.ptype == 0x42) {
        return Err("Disque dynamique : convertissez-le d'abord en disque de base.".into());
    }
    let mut rng = XorShift(seed());
    let mut entries = vec![0u8; (ENTRY_COUNT * ENTRY_SIZE) as usize];
    for (i, p) in parts.iter().enumerate() {
        let (tguid, name, attrs) = match p.ptype {
            0xEF => (GUID_EFI, "EFI system partition", 0u64),
            0x27 => (GUID_RECOVERY, "Recovery", 0x8000_0000_0000_0001u64),
            0x83 => (GUID_LINUX, "Linux filesystem", 0),
            0x82 => (GUID_LINUX_SWAP, "Linux swap", 0),
            _ => (GUID_BASIC, "Basic data partition", 0),
        };
        let e = i * ENTRY_SIZE as usize;
        entries[e..e + 16].copy_from_slice(&guid_bytes(tguid));
        entries[e + 16..e + 32].copy_from_slice(&new_guid(&mut rng));
        entries[e + 32..e + 40].copy_from_slice(&p.start.to_le_bytes());
        entries[e + 40..e + 48].copy_from_slice(&(p.start + p.count - 1).to_le_bytes());
        entries[e + 48..e + 56].copy_from_slice(&attrs.to_le_bytes());
        let n = utf16_name(name);
        entries[e + 56..e + 56 + n.len()].copy_from_slice(&n);
    }
    write_gpt(&mut dev, ss, total, new_guid(&mut rng), &entries)?;
    dev.write_at(0, &protective_mbr(&s0, total))?;
    dev.update_properties();
    Ok(())
}

struct GptPart {
    tguid: [u8; 16],
    first: u64,
    last: u64,
}

fn read_gpt(dev: &mut Dev, ss: u64) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut h = vec![0u8; ss as usize];
    dev.read_at(ss, &mut h)?;
    if &h[0..8] != b"EFI PART" {
        return Err("Aucune table GPT valide n'a été trouvée.".into());
    }
    let elba = u64le(&h, 72);
    let n = u32le(&h, 80) as u64;
    let sz = u32le(&h, 84) as u64;
    if n == 0 || n > 1024 || sz < 128 || sz > 1024 {
        return Err("En-tête GPT inattendu.".into());
    }
    let mut e = vec![0u8; (n * sz).div_ceil(ss) as usize * ss as usize];
    dev.read_at(elba * ss, &mut e)?;
    e.truncate((n * sz) as usize);
    Ok((h, e))
}

/// GPT to MBR without touching the data (at most 4 partitions, all below 2^32 sectors).
pub fn gpt_to_mbr(disk: u32, ss: u64, fat32_offsets: &[u64]) -> Result<(), String> {
    let mut dev = Dev::disk(disk, true)?;
    let total = dev.len()? / ss;
    let (h, e) = read_gpt(&mut dev, ss)?;
    let sz = u32le(&h, 84) as usize;
    let msr = guid_bytes(GUID_MSR);
    let mut parts = Vec::new();
    for chunk in e.chunks(sz) {
        let t: [u8; 16] = chunk[0..16].try_into().unwrap();
        if t == [0u8; 16] || t == msr {
            continue;
        }
        if t == guid_bytes(GUID_LDM_META) || t == guid_bytes(GUID_LDM_DATA) {
            return Err("Disque dynamique : convertissez-le d'abord en disque de base.".into());
        }
        parts.push(GptPart { tguid: t, first: u64le(chunk, 32), last: u64le(chunk, 40) });
    }
    parts.sort_by_key(|p| p.first);
    if parts.len() > 4 {
        return Err("Un disque MBR accepte au plus 4 partitions principales.".into());
    }
    if parts.iter().any(|p| p.last >= 0xFFFF_FFFF) {
        return Err("Une partition dépasse la limite de 2 To d'un disque MBR.".into());
    }
    let mut m = vec![0u8; ss as usize];
    let mut rng = XorShift(seed());
    let mut sig = [0u8; 4];
    rng.fill(&mut sig);
    m[440..444].copy_from_slice(&sig);
    for (i, p) in parts.iter().enumerate() {
        let ptype = if p.tguid == guid_bytes(GUID_EFI) {
            0xEF
        } else if p.tguid == guid_bytes(GUID_RECOVERY) {
            0x27
        } else if p.tguid == guid_bytes(GUID_LINUX) {
            0x83
        } else if p.tguid == guid_bytes(GUID_LINUX_SWAP) {
            0x82
        } else if fat32_offsets.contains(&(p.first * ss)) {
            0x0C
        } else {
            0x07
        };
        let o = 446 + i * 16;
        m[o + 1..o + 4].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
        m[o + 4] = ptype;
        m[o + 5..o + 8].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
        m[o + 8..o + 12].copy_from_slice(&(p.first as u32).to_le_bytes());
        m[o + 12..o + 16].copy_from_slice(&((p.last - p.first + 1) as u32).to_le_bytes());
    }
    m[510] = 0x55;
    m[511] = 0xAA;
    // Remove both GPT headers first, then write the new MBR.
    let zero = vec![0u8; ss as usize];
    dev.write_at(ss, &zero)?;
    let alt = u64le(&h, 32);
    if alt > 1 && alt < total {
        dev.write_at(alt * ss, &zero)?;
    }
    dev.write_at((total - 1) * ss, &zero)?;
    dev.write_at(0, &m)?;
    dev.update_properties();
    Ok(())
}

/// After a clone: new disk identifiers, and the backup GPT moved to the end of the (bigger) disk.
pub fn fix_ids(disk: u32, ss: u64) -> Result<(), String> {
    let mut dev = Dev::disk(disk, true)?;
    let total = dev.len()? / ss;
    let mut rng = XorShift(seed());
    let mut s0 = vec![0u8; ss as usize];
    dev.read_at(0, &mut s0)?;
    if let Ok((h, mut e)) = read_gpt(&mut dev, ss) {
        let n = u32le(&h, 80) as u64;
        let sz = u32le(&h, 84) as u64;
        if n != ENTRY_COUNT || sz != ENTRY_SIZE {
            return Err("Format GPT non standard : identifiants laissés tels quels.".into());
        }
        for chunk in e.chunks_mut(sz as usize) {
            if chunk[0..16] != [0u8; 16] {
                chunk[16..32].copy_from_slice(&new_guid(&mut rng));
            }
        }
        write_gpt(&mut dev, ss, total, new_guid(&mut rng), &e)?;
        dev.write_at(0, &protective_mbr(&s0, total))?;
    } else {
        let mut sig = [0u8; 4];
        rng.fill(&mut sig);
        s0[440..444].copy_from_slice(&sig);
        dev.write_at(0, &s0)?;
    }
    dev.update_properties();
    Ok(())
}

/// After a partition's data has been moved: point its table entry at the new start
/// (same length, same identifiers).
pub fn move_entry(disk: u32, ss: u64, old_lba: u64, new_lba: u64) -> Result<(), String> {
    let mut dev = Dev::disk(disk, true)?;
    let total = dev.len()? / ss;
    if let Ok((h, mut e)) = read_gpt(&mut dev, ss) {
        let n = u32le(&h, 80) as u64;
        let sz = u32le(&h, 84) as u64;
        if n != ENTRY_COUNT || sz != ENTRY_SIZE {
            return Err("Format GPT non standard : déplacement refusé.".into());
        }
        let mut found = false;
        for chunk in e.chunks_mut(sz as usize) {
            if chunk[0..16] != [0u8; 16] && u64le(chunk, 32) == old_lba {
                let len = u64le(chunk, 40) - old_lba + 1;
                chunk[32..40].copy_from_slice(&new_lba.to_le_bytes());
                chunk[40..48].copy_from_slice(&(new_lba + len - 1).to_le_bytes());
                found = true;
            }
        }
        if !found {
            return Err("Entrée de la partition introuvable dans la table GPT.".into());
        }
        let guid: [u8; 16] = h[56..72].try_into().unwrap();
        write_gpt(&mut dev, ss, total, guid, &e)?;
    } else {
        let mut s0 = vec![0u8; ss as usize];
        dev.read_at(0, &mut s0)?;
        let mut found = false;
        for i in 0..4 {
            let o = 446 + i * 16;
            if s0[o + 4] != 0 && u32le(&s0, o + 8) as u64 == old_lba {
                if new_lba > 0xFFFF_FFFF {
                    return Err("Position hors de portée d'un disque MBR.".into());
                }
                s0[o + 1..o + 4].copy_from_slice(&[0xFE, 0xFF, 0xFF]);
                s0[o + 8..o + 12].copy_from_slice(&(new_lba as u32).to_le_bytes());
                found = true;
            }
        }
        if !found {
            return Err("Entrée de la partition introuvable dans la table MBR (les partitions logiques ne peuvent pas être déplacées).".into());
        }
        dev.write_at(0, &s0)?;
    }
    dev.update_properties();
    Ok(())
}

/// Boot sectors record where their partition starts ("hidden sectors"): fix them after a move.
pub fn fix_boot_offsets(dev: &mut Dev, start: u64, size: u64, ss: u64) -> Result<(), String> {
    let lba = start / ss;
    let mut b = vec![0u8; ss as usize];
    dev.read_at(start, &mut b)?;
    if &b[3..11] == b"NTFS    " {
        for pos in [start, start + size - ss] {
            dev.read_at(pos, &mut b)?;
            if &b[3..11] == b"NTFS    " {
                b[0x1C..0x20].copy_from_slice(&(lba.min(0xFFFF_FFFF) as u32).to_le_bytes());
                dev.write_at(pos, &b)?;
            }
        }
    } else if &b[82..90] == b"FAT32   " || &b[54..62] == b"FAT16   " || &b[54..62] == b"FAT12   " {
        for sec in [0u64, 6] {
            dev.read_at(start + sec * ss, &mut b)?;
            if b[510] == 0x55 && b[511] == 0xAA {
                b[0x1C..0x20].copy_from_slice(&(lba.min(0xFFFF_FFFF) as u32).to_le_bytes());
                dev.write_at(start + sec * ss, &b)?;
            }
        }
    } else if &b[3..11] == b"EXFAT   " {
        // Main and backup boot regions: 12 sectors each, the 12th holds a checksum of the first 11.
        for region in [0u64, 12] {
            let base = start + region * ss;
            let mut r = vec![0u8; (12 * ss) as usize];
            dev.read_at(base, &mut r)?;
            if &r[3..11] != b"EXFAT   " {
                continue;
            }
            r[0x40..0x48].copy_from_slice(&lba.to_le_bytes());
            let mut c: u32 = 0;
            for (i, &byte) in r[..(11 * ss) as usize].iter().enumerate() {
                if i == 106 || i == 107 || i == 112 {
                    continue;
                }
                c = c.rotate_right(1).wrapping_add(byte as u32);
            }
            for k in (11 * ss as usize..12 * ss as usize).step_by(4) {
                r[k..k + 4].copy_from_slice(&c.to_le_bytes());
            }
            dev.write_at(base, &r)?;
        }
    }
    Ok(())
}
