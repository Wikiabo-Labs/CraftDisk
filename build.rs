//! Turns assets/logo.svg into the program's icon (the one Windows shows for DiskCraft.exe).
//! No resource compiler is needed: the icon resource object is written here directly.

use std::path::PathBuf;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn render(svg: &[u8], size: u32) -> Option<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pix = resvg::tiny_skia::Pixmap::new(size, size)?;
    let s = size as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pix.as_mut());
    pix.encode_png().ok()
}

fn dir(named: u16, ids: u16) -> Vec<u8> {
    let mut v = vec![0u8; 12];
    v.extend_from_slice(&named.to_le_bytes());
    v.extend_from_slice(&ids.to_le_bytes());
    v
}
fn entry(v: &mut Vec<u8>, id: u32, off: u32, subdir: bool) {
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(&(if subdir { off | 0x8000_0000 } else { off }).to_le_bytes());
}

/// A COFF object holding RT_ICON entries 1..n and RT_GROUP_ICON 1.
fn icon_object(pngs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let n = pngs.len() as u32;
    // Layout of the directory part.
    let root = 0u32;
    let type_icon = root + 16 + 2 * 8;
    let type_group = type_icon + 16 + n * 8;
    let name_icons = type_group + 16 + 8; // n dirs of 24 bytes
    let name_group = name_icons + n * 24;
    let data_entries = name_group + 24; // n + 1 entries of 16 bytes
    let blobs = (data_entries + (n + 1) * 16 + 7) & !7;

    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(n as u16).to_le_bytes());
    for (i, (size, png)) in pngs.iter().enumerate() {
        let b = if *size >= 256 { 0 } else { *size as u8 };
        group.extend_from_slice(&[b, b, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&((i + 1) as u16).to_le_bytes());
    }

    let mut s = Vec::new();
    s.extend(dir(0, 2));
    entry(&mut s, 3, type_icon, true);
    entry(&mut s, 14, type_group, true);
    s.extend(dir(0, n as u16));
    for i in 0..n {
        entry(&mut s, i + 1, name_icons + i * 24, true);
    }
    s.extend(dir(0, 1));
    entry(&mut s, 1, name_group, true);
    for i in 0..n {
        s.extend(dir(0, 1));
        entry(&mut s, 1033, data_entries + i * 16, false);
    }
    s.extend(dir(0, 1));
    entry(&mut s, 1033, data_entries + n * 16, false);

    // Data entries (their first field is relocated to an RVA by the linker).
    let mut relocs = Vec::new();
    let mut blob_off = blobs;
    let mut blob_list: Vec<&[u8]> = pngs.iter().map(|(_, p)| p.as_slice()).collect();
    blob_list.push(&group);
    let mut offsets = Vec::new();
    for b in &blob_list {
        offsets.push(blob_off);
        blob_off = (blob_off + b.len() as u32 + 7) & !7;
    }
    for (b, off) in blob_list.iter().zip(&offsets) {
        relocs.push(s.len() as u32);
        s.extend_from_slice(&off.to_le_bytes());
        s.extend_from_slice(&(b.len() as u32).to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
    }
    for (b, off) in blob_list.iter().zip(&offsets) {
        s.resize(*off as usize, 0);
        s.extend_from_slice(b);
    }
    s.resize(((s.len() + 7) & !7) as usize, 0);

    let raw_ptr = 20u32 + 40;
    let reloc_ptr = raw_ptr + s.len() as u32;
    let sym_ptr = reloc_ptr + relocs.len() as u32 * 10;
    let mut o = Vec::new();
    o.extend_from_slice(&0x8664u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&sym_ptr.to_le_bytes());
    o.extend_from_slice(&1u32.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(b".rsrc\0\0\0");
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&(s.len() as u32).to_le_bytes());
    o.extend_from_slice(&raw_ptr.to_le_bytes());
    o.extend_from_slice(&reloc_ptr.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&(relocs.len() as u16).to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0x4030_0040u32.to_le_bytes());
    o.extend_from_slice(&s);
    for r in &relocs {
        o.extend_from_slice(&r.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes());
        o.extend_from_slice(&3u16.to_le_bytes()); // IMAGE_REL_AMD64_ADDR32NB
    }
    o.extend_from_slice(b".rsrc\0\0\0");
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&1i16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.push(3); // static
    o.push(0);
    o.extend_from_slice(&4u32.to_le_bytes());
    o
}

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.svg");
    println!("cargo:rerun-if-changed=build.rs");
    let windows_gnu = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    if !windows_gnu {
        return;
    }
    let Ok(svg) = std::fs::read("assets/logo.svg") else { return };
    let pngs: Vec<(u32, Vec<u8>)> = SIZES.iter().filter_map(|&s| render(&svg, s).map(|p| (s, p))).collect();
    if pngs.len() != SIZES.len() {
        println!("cargo:warning=logo could not be rendered; the program keeps the default icon");
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("diskcraft_icon.o");
    if std::fs::write(&out, icon_object(&pngs)).is_ok() {
        println!("cargo:rustc-link-arg-bins={}", out.display());
    }
}
