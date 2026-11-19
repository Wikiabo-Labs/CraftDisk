//! The DiskCraft logo, drawn from one file (assets/logo.svg) everywhere: window icon,
//! taskbar, program icon (build.rs) and inside the app.

pub const SVG: &[u8] = include_bytes!("../assets/logo.svg");

/// The logo as RGBA pixels (straight alpha) at `size` x `size`.
pub fn rgba(size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (size * size * 4) as usize];
    let Ok(tree) = resvg::usvg::Tree::from_data(SVG, &resvg::usvg::Options::default()) else { return out };
    let Some(mut pix) = resvg::tiny_skia::Pixmap::new(size, size) else { return out };
    let s = size as f32 / tree.size().width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pix.as_mut());
    for (i, p) in pix.pixels().iter().enumerate() {
        let c = p.demultiply();
        out[i * 4..i * 4 + 4].copy_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

pub fn icon() -> egui::IconData {
    egui::IconData { rgba: rgba(256), width: 256, height: 256 }
}

pub fn image(size: u32) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], &rgba(size))
}

/// `--export-logo <folder>`: PNG files at the usual sizes and a Windows .ico.
pub fn export(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let sizes = [16u32, 24, 32, 48, 64, 128, 256, 512, 1024];
    let mut ico_parts = Vec::new();
    for s in sizes {
        let path = format!("{dir}/diskcraft-{s}.png");
        image::save_buffer(&path, &rgba(s), s, s, image::ColorType::Rgba8).map_err(|e| e.to_string())?;
        if s <= 256 {
            ico_parts.push((s, std::fs::read(&path).map_err(|e| e.to_string())?));
        }
    }
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(ico_parts.len() as u16).to_le_bytes());
    let mut off = 6 + 16 * ico_parts.len() as u32;
    for (s, png) in &ico_parts {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        ico.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&off.to_le_bytes());
        off += png.len() as u32;
    }
    for (_, png) in &ico_parts {
        ico.extend_from_slice(png);
    }
    std::fs::write(format!("{dir}/diskcraft.ico"), ico).map_err(|e| e.to_string())?;
    std::fs::write(format!("{dir}/diskcraft.svg"), SVG).map_err(|e| e.to_string())
}
