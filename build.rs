//! Windows resources of DiskCraft.exe: the icon (from assets/logo.svg) and the application
//! manifest asking for administrator rights at start. No resource compiler is needed: the
//! resource object is written here directly.

use std::path::PathBuf;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const RT_MANIFEST: u32 = 24;
const LANG_EN_US: u32 = 1033;

/// The application manifest: DiskCraft always starts with administrator rights
/// (Windows asks once, when the program opens), and declares Windows 10/11 support.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#;

/// One resource: type, numeric id, bytes.
struct Res {
    typ: u32,
    id: u32,
    data: Vec<u8>,
}

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

/// RT_ICON entries 1..n and the RT_GROUP_ICON 1 that lists them.
fn icon_resources(pngs: &[(u32, Vec<u8>)]) -> Vec<Res> {
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(pngs.len() as u16).to_le_bytes());
    for (i, (size, png)) in pngs.iter().enumerate() {
        let b = if *size >= 256 { 0 } else { *size as u8 };
        group.extend_from_slice(&[b, b, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&((i + 1) as u16).to_le_bytes());
    }
    let mut out: Vec<Res> = pngs.iter().enumerate().map(|(i, (_, png))| Res { typ: RT_ICON, id: i as u32 + 1, data: png.clone() }).collect();
    out.push(Res { typ: RT_GROUP_ICON, id: 1, data: group });
    out
}

/// A COFF object with one .rsrc section holding `res` (type → id → one language each).
fn rsrc_object(mut res: Vec<Res>) -> Vec<u8> {
    // Entries of a resource directory must be sorted by id.
    res.sort_by_key(|r| (r.typ, r.id));
    let mut types: Vec<u32> = res.iter().map(|r| r.typ).collect();
    types.dedup();

    // Layout: root, one directory per type, one language directory per resource, data entries, blobs.
    let mut off = 16 + 8 * types.len() as u32;
    let mut type_dir = Vec::new();
    for t in &types {
        type_dir.push(off);
        off += 16 + 8 * res.iter().filter(|r| r.typ == *t).count() as u32;
    }
    let mut lang_dir = Vec::new();
    for _ in &res {
        lang_dir.push(off);
        off += 24;
    }
    let mut data_entry = Vec::new();
    for _ in &res {
        data_entry.push(off);
        off += 16;
    }
    let mut blob = Vec::new();
    let mut blob_off = (off + 7) & !7;
    for r in &res {
        blob.push(blob_off);
        blob_off = (blob_off + r.data.len() as u32 + 7) & !7;
    }

    let mut s = Vec::new();
    s.extend(dir(0, types.len() as u16));
    for (t, o) in types.iter().zip(&type_dir) {
        entry(&mut s, *t, *o, true);
    }
    for t in &types {
        let idx: Vec<usize> = (0..res.len()).filter(|&i| res[i].typ == *t).collect();
        s.extend(dir(0, idx.len() as u16));
        for i in idx {
            entry(&mut s, res[i].id, lang_dir[i], true);
        }
    }
    for de in &data_entry {
        s.extend(dir(0, 1));
        entry(&mut s, LANG_EN_US, *de, false);
    }
    // Data entries (their first field is relocated to an RVA by the linker).
    let mut relocs = Vec::new();
    for (r, b) in res.iter().zip(&blob) {
        relocs.push(s.len() as u32);
        s.extend_from_slice(&b.to_le_bytes());
        s.extend_from_slice(&(r.data.len() as u32).to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
    }
    for (r, b) in res.iter().zip(&blob) {
        s.resize(*b as usize, 0);
        s.extend_from_slice(&r.data);
    }
    s.resize((s.len() + 7) & !7, 0);

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
    println!("cargo:rerun-if-env-changed=PROFILE");
    // With the GNU toolchain no resource compiler is needed: the resource object is written here.
    // (An MSVC build gets neither the icon nor the administrator manifest.)
    let windows_gnu = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    if !windows_gnu {
        return;
    }
    let mut res = Vec::new();
    // Release builds only: debug builds (cargo run, cargo test) stay runnable from a normal terminal
    // and ask for administrator rights when an operation runs.
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        res.push(Res { typ: RT_MANIFEST, id: 1, data: MANIFEST.as_bytes().to_vec() });
    }
    let pngs: Vec<(u32, Vec<u8>)> = match std::fs::read("assets/logo.svg") {
        Ok(svg) => SIZES.iter().filter_map(|&s| render(&svg, s).map(|p| (s, p))).collect(),
        Err(_) => Vec::new(),
    };
    if pngs.len() == SIZES.len() {
        res.extend(icon_resources(&pngs));
    } else {
        println!("cargo:warning=logo could not be rendered; the program keeps the default icon");
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("diskcraft_resources.o");
    if std::fs::write(&out, rsrc_object(res)).is_ok() {
        println!("cargo:rustc-link-arg-bins={}", out.display());
    }
}
