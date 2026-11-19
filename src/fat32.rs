//! FAT32 writer for volumes of any size up to 2 TiB (Windows' own tools stop at 32 GB),
//! written from the published FAT32 layout.

use crate::ops::fat32_default_cluster;
use crate::rawio::{seed, Dev, XorShift};

pub fn format(dev: &mut Dev, size: u64, ss: u64, hidden: u64, label: &str, cluster: u32, progress: &mut dyn FnMut(f32)) -> Result<(), String> {
    let total = size / ss;
    if total > 0xFFFF_FFFF {
        return Err("FAT32 est limité à 2^32 secteurs (2 To).".into());
    }
    let cluster = if cluster == 0 { fat32_default_cluster(size) } else { cluster } as u64;
    let spc = (cluster / ss).max(1);
    if spc > 128 || !spc.is_power_of_two() {
        return Err("Taille de cluster non valide pour FAT32.".into());
    }
    let reserved: u64 = 32;
    let nfats: u64 = 2;
    // FAT size in sectors: 4 bytes per cluster, rounded up.
    let fat_secs = (4 * (total - reserved)) / (spc * ss + 4 * nfats) + 1;
    let fat_secs = fat_secs.div_ceil(1).max(1);
    let data_start = reserved + nfats * fat_secs;
    if data_start >= total {
        return Err("Volume trop petit pour FAT32.".into());
    }
    let clusters = (total - data_start) / spc;
    if clusters < crate::model::FAT32_MIN_CLUSTERS {
        return Err("Volume trop petit pour FAT32 avec cette taille de cluster : choisissez un cluster plus petit.".into());
    }
    if clusters > 0x0FFF_FFF5 {
        return Err("Trop de clusters : choisissez un cluster plus grand.".into());
    }
    let s = ss as usize;
    let mut vol_id = [0u8; 4];
    XorShift(seed()).fill(&mut vol_id);
    let mut lab = [b' '; 11];
    for (i, c) in label.to_uppercase().bytes().filter(|b| b.is_ascii_graphic() || *b == b' ').take(11).enumerate() {
        lab[i] = c;
    }
    let lab = if label.trim().is_empty() { *b"NO NAME    " } else { lab };

    // Boot sector.
    let mut bs = vec![0u8; s];
    bs[0..3].copy_from_slice(&[0xEB, 0x58, 0x90]);
    bs[3..11].copy_from_slice(b"MSWIN4.1");
    bs[11..13].copy_from_slice(&(ss as u16).to_le_bytes());
    bs[13] = spc as u8;
    bs[14..16].copy_from_slice(&(reserved as u16).to_le_bytes());
    bs[16] = nfats as u8;
    bs[21] = 0xF8;
    bs[24..26].copy_from_slice(&63u16.to_le_bytes());
    bs[26..28].copy_from_slice(&255u16.to_le_bytes());
    bs[28..32].copy_from_slice(&(hidden.min(0xFFFF_FFFF) as u32).to_le_bytes());
    bs[32..36].copy_from_slice(&(total as u32).to_le_bytes());
    bs[36..40].copy_from_slice(&(fat_secs as u32).to_le_bytes());
    bs[44..48].copy_from_slice(&2u32.to_le_bytes()); // root directory cluster
    bs[48..50].copy_from_slice(&1u16.to_le_bytes()); // FSInfo sector
    bs[50..52].copy_from_slice(&6u16.to_le_bytes()); // backup boot sector
    bs[64] = 0x80;
    bs[66] = 0x29;
    bs[67..71].copy_from_slice(&vol_id);
    bs[71..82].copy_from_slice(&lab);
    bs[82..90].copy_from_slice(b"FAT32   ");
    bs[510] = 0x55;
    bs[511] = 0xAA;
    if s > 512 {
        bs[s - 2] = 0x55;
        bs[s - 1] = 0xAA;
    }

    // FSInfo sector.
    let mut fsi = vec![0u8; s];
    fsi[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
    fsi[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
    fsi[488..492].copy_from_slice(&((clusters - 1) as u32).to_le_bytes());
    fsi[492..496].copy_from_slice(&3u32.to_le_bytes());
    fsi[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());

    // Clear reserved area, FATs and the root cluster.
    let clear_secs = data_start + spc;
    let chunk_secs = (4 * 1024 * 1024 / ss).max(1);
    let zero = vec![0u8; (chunk_secs * ss) as usize];
    let mut done = 0u64;
    while done < clear_secs {
        let n = chunk_secs.min(clear_secs - done);
        dev.write_at(done * ss, &zero[..(n * ss) as usize])?;
        done += n;
        progress(done as f32 / clear_secs as f32 * 0.95);
    }

    let mut third = vec![0u8; s];
    third[510] = 0x55;
    third[511] = 0xAA;
    for base in [0u64, 6] {
        dev.write_at(base * ss, &bs)?;
        dev.write_at((base + 1) * ss, &fsi)?;
        dev.write_at((base + 2) * ss, &third)?;
    }
    let mut fat0 = vec![0u8; s];
    fat0[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
    fat0[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
    fat0[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // root directory: end of chain
    for f in 0..nfats {
        dev.write_at((reserved + f * fat_secs) * ss, &fat0)?;
    }
    // Volume label entry in the root directory.
    let mut root = vec![0u8; s];
    root[0..11].copy_from_slice(&lab);
    root[11] = 0x08;
    dev.write_at(data_start * ss, &root)?;
    progress(1.0);
    Ok(())
}
