//! The DiskCraft window.

use crate::model::*;
use crate::ops::{self, Op};
use crate::plan::{Plan, Step};
use crate::winpe;
use egui::{Align2, Color32, FontId, RichText, Rounding, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

// Colours.
/// Light or dark palette, chosen each frame from the theme egui resolved
/// (Settings: light, dark or follow Windows).
static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn pick(dark: Color32, light: Color32) -> Color32 {
    if LIGHT.load(std::sync::atomic::Ordering::Relaxed) {
        light
    } else {
        dark
    }
}
#[allow(non_snake_case)]
fn NAV() -> Color32 {
    pick(Color32::from_rgb(0x1a, 0x1b, 0x1f), Color32::from_rgb(0xee, 0xf0, 0xf4))
}
#[allow(non_snake_case)]
fn NAV_HI() -> Color32 {
    pick(Color32::from_rgb(0x2b, 0x2d, 0x33), Color32::from_rgb(0xdf, 0xe3, 0xea))
}
const ACCENT: Color32 = Color32::from_rgb(0x4a, 0x7d, 0xf7);
#[allow(non_snake_case)]
fn ACCENT_DARK() -> Color32 {
    pick(Color32::from_rgb(0x8f, 0xb0, 0xff), Color32::from_rgb(0x2f, 0x5f, 0xd0))
}
const BRAND: Color32 = Color32::from_rgb(0x22, 0xa0, 0x78);
#[allow(non_snake_case)]
fn BG() -> Color32 {
    pick(Color32::from_rgb(0x20, 0x21, 0x25), Color32::from_rgb(0xf6, 0xf7, 0xf9))
}
#[allow(non_snake_case)]
fn CARD() -> Color32 {
    pick(Color32::from_rgb(0x28, 0x29, 0x2e), Color32::from_rgb(0xff, 0xff, 0xff))
}
#[allow(non_snake_case)]
fn CARD_HI() -> Color32 {
    pick(Color32::from_rgb(0x31, 0x33, 0x39), Color32::from_rgb(0xee, 0xf0, 0xf3))
}
#[allow(non_snake_case)]
fn LINE() -> Color32 {
    pick(Color32::from_rgb(0x38, 0x3a, 0x41), Color32::from_rgb(0xdc, 0xdf, 0xe5))
}
#[allow(non_snake_case)]
fn TEXT() -> Color32 {
    pick(Color32::from_rgb(0xe8, 0xea, 0xee), Color32::from_rgb(0x1d, 0x20, 0x27))
}
#[allow(non_snake_case)]
fn MUTED() -> Color32 {
    pick(Color32::from_rgb(0x9b, 0xa1, 0xab), Color32::from_rgb(0x5d, 0x64, 0x70))
}

#[allow(non_snake_case)]
fn NAV_TEXT() -> Color32 {
    pick(Color32::from_gray(200), Color32::from_rgb(0x3a, 0x40, 0x4a))
}
/// Empty bar background and hatching of unallocated space.
#[allow(non_snake_case)]
fn TRACK() -> Color32 {
    pick(Color32::from_rgb(0x2c, 0x2e, 0x34), Color32::from_rgb(0xe6, 0xe9, 0xee))
}
#[allow(non_snake_case)]
fn HATCH() -> Color32 {
    pick(Color32::from_rgb(0x42, 0x45, 0x4d), Color32::from_rgb(0xc9, 0xce, 0xd6))
}
#[allow(non_snake_case)]
fn INFO_BG() -> Color32 {
    pick(Color32::from_rgb(0x22, 0x30, 0x4a), Color32::from_rgb(0xe4, 0xec, 0xfd))
}
#[allow(non_snake_case)]
fn PENDING_BG() -> Color32 {
    pick(Color32::from_rgb(0x33, 0x2b, 0x20), Color32::from_rgb(0xfd, 0xf0, 0xdc))
}
const PENDING: Color32 = Color32::from_rgb(0xf0, 0xa0, 0x3c);
const DANGER: Color32 = Color32::from_rgb(0xf0, 0x6a, 0x6a);
const ICON_BLUE: Color32 = Color32::from_rgb(0x6c, 0x9b, 0xff);
const ICON_GREEN: Color32 = Color32::from_rgb(0x4c, 0xc3, 0x8a);
const ICON_ORANGE: Color32 = Color32::from_rgb(0xf2, 0xa3, 0x4b);
const ICON_PURPLE: Color32 = Color32::from_rgb(0xb0, 0x8c, 0xf5);
const ICON_RED: Color32 = Color32::from_rgb(0xf0, 0x6d, 0x7a);
const ICON_TEAL: Color32 = Color32::from_rgb(0x3f, 0xc6, 0xc9);

fn fs_color(p: &Partition) -> Color32 {
    match p.kind {
        Kind::Efi => Color32::from_rgb(0x8a, 0x63, 0xd2),
        Kind::Msr => Color32::from_rgb(0x9a, 0xa3, 0xb2),
        Kind::Recovery => Color32::from_rgb(0xd9, 0x8b, 0x2b),
        Kind::LdmMeta | Kind::LdmData => Color32::from_rgb(0x4d, 0x7c, 0xb8),
        _ => match p.fs {
            Fs::Ntfs => Color32::from_rgb(0x2f, 0x7f, 0xd8),
            Fs::Fat32 | Fs::Fat => Color32::from_rgb(0x3f, 0xa6, 0x5a),
            Fs::ExFat => Color32::from_rgb(0x1f, 0xa3, 0xa8),
            Fs::Refs => Color32::from_rgb(0x5a, 0x5f, 0xcf),
            Fs::Raw => Color32::from_rgb(0xa0, 0x82, 0x6a),
            Fs::Other(_) => Color32::from_rgb(0x7b, 0x84, 0x93),
        },
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Home,
    Health,
    Partitions,
    Clone,
    Migrate,
    Convert,
    WinPe,
    Support,
    Settings,
    About,
}

#[derive(Clone, Copy, PartialEq)]
enum Sel {
    None,
    Disk(u32),
    Part(u32, u64),
    Free(u32, u64, u64),
}

enum Dlg {
    Resize { disk: u32, offset: u64, pos: u64, size_mb: f64, regions: Vec<(u64, u64)>, min: u64, cur: u64, can_resize: bool, can_move: bool, grab: Option<u64> },
    Create { disk: u32, offset: u64, max: u64, size_mb: f64, fs: Fs, label: String, letter: Option<char> },
    Format { disk: u32, offset: u64, fs: Fs, label: String, cluster: u32, quick: bool },
    Delete { disk: u32, offset: u64 },
    Wipe { disk: u32, offset: Option<u64>, passes: u8 },
    Merge { disk: u32, first: u64 },
    Letter { disk: u32, offset: u64, letter: Option<char> },
    Label { disk: u32, offset: u64, label: String },
    Check { disk: u32, offset: u64, mode: usize },
    Confirm { backup_ok: bool },
    Message { title: String, text: String },
}

enum Act {
    Select(Sel),
    Open(Dlg),
    Add(Op),
    Undo,
    UndoAll,
    Execute,
    RunNow(Vec<Step>, String),
    Reload,
    Page(Page),
}

struct Run {
    log: PathBuf,
    title: String,
    lines: Vec<String>,
    step: String,
    pct: f32,
    done: Option<bool>,
    queue: bool,
    last_read: f64,
}

struct Shot {
    path: String,
    frames: u32,
    asked: bool,
}

pub struct App {
    logo: egui::TextureHandle,
    base: Vec<Disk>,
    preview: Vec<Disk>,
    ops: Vec<Op>,
    demo: bool,
    loading: Option<Receiver<Result<Vec<Disk>, String>>>,
    health: std::collections::HashMap<u32, Health>,
    health_loading: Option<Receiver<Result<std::collections::HashMap<u32, Health>, String>>>,
    load_note: Option<String>,
    page: Page,
    sel: Sel,
    dialog: Option<Dlg>,
    run: Option<Run>,
    shot: Option<Shot>,
    started: f64,
    clone_part_mode: bool,
    clone_src: Option<u32>,
    clone_dst: Option<u32>,
    clone_psrc: Option<(u32, u64)>,
    clone_pdst: Option<(u32, u64)>,
    fit: bool,
    mig_dst: Option<u32>,
    conv_disk: Option<u32>,
    fat_part: Option<(u32, u64)>,
    temp_drive: Option<char>,
    dyn_disk: Option<u32>,
    pe_iso: bool,
    pe_usb: Option<char>,
    pe_path: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<String>, demo: bool) -> App {
        // One DiskCraft style per theme; egui picks the one matching the Settings choice
        // (light, dark or follow Windows) and the palette functions follow it.
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            LIGHT.store(theme == egui::Theme::Light, std::sync::atomic::Ordering::Relaxed);
            let light = theme == egui::Theme::Light;
            let mut style = (*cc.egui_ctx.style_of(theme)).clone();
            style.visuals = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
            style.visuals.extreme_bg_color = pick(Color32::from_rgb(0x1b, 0x1c, 0x20), Color32::from_rgb(0xfb, 0xfc, 0xfd));
            style.visuals.faint_bg_color = TRACK();
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE());
            style.visuals.widgets.inactive.weak_bg_fill = CARD_HI();
            style.visuals.widgets.inactive.bg_fill = CARD_HI();
            style.visuals.widgets.hovered.weak_bg_fill = pick(Color32::from_rgb(0x3a, 0x3d, 0x45), Color32::from_rgb(0xe2, 0xe6, 0xec));
            style.visuals.widgets.active.weak_bg_fill = pick(Color32::from_rgb(0x44, 0x48, 0x52), Color32::from_rgb(0xd5, 0xda, 0xe2));
            style.visuals.window_stroke = Stroke::new(1.0_f32, LINE());
            style.visuals.hyperlink_color = ACCENT_DARK();
            style.visuals.panel_fill = BG();
            style.visuals.window_fill = CARD();
            style.visuals.override_text_color = Some(TEXT());
            style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.6);
            style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_DARK());
            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
            style.visuals.window_rounding = Rounding::same(10.0);
            style.spacing.item_spacing = Vec2::new(8.0, 7.0);
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
            cc.egui_ctx.set_style_of(theme, style);
        }
        cc.egui_ctx.set_theme(theme_pref::load().egui());
        LIGHT.store(cc.egui_ctx.theme() == egui::Theme::Light, std::sync::atomic::Ordering::Relaxed);
        let logo_tex = cc.egui_ctx.load_texture("logo", crate::logo::image(128), egui::TextureOptions::LINEAR);

        let docs = std::env::var("USERPROFILE").map(|p| format!(r"{p}\Documents\DiskCraft-WinPE.iso")).unwrap_or_else(|_| r"C:\DiskCraft-WinPE.iso".into());
        let mut app = App {
            base: Vec::new(),
            preview: Vec::new(),
            ops: Vec::new(),
            demo,
            loading: None,
            health: std::collections::HashMap::new(),
            health_loading: None,
            load_note: None,
            page: Page::Home,
            logo: logo_tex,
            sel: Sel::None,
            dialog: None,
            run: None,
            shot: screenshot.map(|path| Shot { path, frames: 0, asked: false }),
            started: 0.0,
            clone_part_mode: false,
            clone_src: None,
            clone_dst: None,
            clone_psrc: None,
            clone_pdst: None,
            fit: true,
            mig_dst: None,
            conv_disk: None,
            fat_part: None,
            temp_drive: None,
            dyn_disk: None,
            pe_iso: true,
            pe_usb: None,
            pe_path: docs,
        };
        // Optional start page (used to picture each page): DISKCRAFT_PAGE=partitions|clone|...
        app.page = match std::env::var("DISKCRAFT_PAGE").unwrap_or_default().as_str() {
            "health" => Page::Health,
            "partitions" => Page::Partitions,
            "clone" => Page::Clone,
            "migrate" => Page::Migrate,
            "convert" => Page::Convert,
            "winpe" => Page::WinPe,
            "support" => Page::Support,
            "settings" => Page::Settings,
            "about" => Page::About,
            _ => Page::Home,
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        if self.demo {
            self.set_disks(crate::probe::demo_disks());
            self.health = crate::probe::demo_health();
            // Picture of the move window (checks only): DISKCRAFT_DIALOG=move with --demo.
            if std::env::var("DISKCRAFT_DIALOG").as_deref() == Ok("move") {
                let mut d = self.preview[1].clone();
                d.parts.remove(0); // leave free space before E:
                self.preview[1] = d.clone();
                self.base[1] = d.clone();
                let p = d.parts[0].clone();
                let lo = p.offset - d.free_before(p.offset);
                let hi = p.end() + d.free_after(p.offset);
                let mut regions = vec![(lo, hi)];
                for s in d.segments() {
                    if let Segment::Free { offset: fo, size: fs } = s {
                        if fo + fs <= lo || fo >= hi {
                            regions.push((fo, fo + fs));
                        }
                    }
                }
                self.page = Page::Partitions;
                self.sel = Sel::Part(d.number, p.offset);
                self.dialog = Some(Dlg::Resize { disk: d.number, offset: p.offset, pos: p.offset, size_mb: (p.size / MIB) as f64, regions, min: ops::min_size(&p), cur: p.size, can_resize: true, can_move: true, grab: None });
            }
            // DISKCRAFT_CLEAN_SHOT: pictures for the README, without the demo banner.
            self.load_note = if std::env::var("DISKCRAFT_CLEAN_SHOT").is_ok() {
                None
            } else {
                Some(crate::tr!("Mode démonstration : ce sont des disques d'exemple, rien n'est modifié sur votre PC.", "Demo mode: these are example disks, nothing is changed on your PC.").into())
            };
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::probe::load_disks());
        });
        self.loading = Some(rx);
        // SMART values are read at the same time as the disks, in their own thread (slower).
        let (htx, hrx) = channel();
        std::thread::spawn(move || {
            let _ = htx.send(crate::probe::load_health());
        });
        self.health_loading = Some(hrx);
    }

    fn set_disks(&mut self, d: Vec<Disk>) {
        self.base = d;
        self.ops.clear();
        self.preview = self.base.clone();
        self.sel = Sel::None;
    }

    fn rebuild_preview(&mut self) -> Result<(), String> {
        let mut p = self.base.clone();
        for op in &self.ops {
            op.apply(&mut p)?;
        }
        self.preview = p;
        Ok(())
    }

    fn add_op(&mut self, op: Op) {
        let mut p = self.preview.clone();
        match op.apply(&mut p) {
            Ok(()) => {
                self.ops.push(op);
                self.preview = p;
            }
            Err(e) => self.dialog = Some(Dlg::Message { title: crate::tr!("Opération impossible", "Operation not possible").into(), text: e }),
        }
    }

    fn plan(&self) -> Result<Vec<Step>, String> {
        let mut state = self.base.clone();
        let mut steps = Vec::new();
        for op in &self.ops {
            steps.extend(op.steps(&state)?);
            op.apply(&mut state)?;
        }
        Ok(steps)
    }

    fn start_worker(&mut self, steps: Vec<Step>, title: String, queue: bool) {
        if self.demo {
            self.dialog = Some(Dlg::Message {
                title: crate::tr!("Mode démonstration", "Demo mode").into(),
                text: crate::tr!("Les disques affichés sont des exemples : rien n'a été exécuté. Relancez DiskCraft sans --demo pour travailler sur vos vrais disques.", "The disks shown are examples: nothing was run. Start DiskCraft without --demo to work on your real disks.").into(),
            });
            return;
        }
        let dir = std::env::temp_dir().join("DiskCraft");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let plan_path = dir.join(format!("plan-{stamp}.json"));
        let log_path = dir.join(format!("journal-{stamp}.txt"));
        let disks = self.base.iter().map(|d| crate::plan::DiskId { number: d.number, size: d.size, uid: d.uid.clone(), model: d.model.clone() }).collect();
        let json = serde_json::to_string_pretty(&Plan { steps, disks }).unwrap_or_default();
        if let Err(e) = std::fs::write(&plan_path, &json) {
            self.dialog = Some(Dlg::Message { title: crate::tr!("Erreur", "Error").into(), text: e.to_string() });
            return;
        }
        let _ = std::fs::write(&log_path, "");
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
        // The plan's fingerprint travels on the worker's command line, which cannot be changed once
        // started: the worker refuses a plan file that was altered meanwhile.
        let hash = crate::worker::sha256_hex(json.as_bytes());
        let lp = log_path.clone();
        if crate::rawio::is_elevated() {
            // DiskCraft already runs as administrator (asked once at start): the worker inherits it.
            let (pp, lp2) = (plan_path.clone(), log_path.clone());
            std::thread::spawn(move || {
                let mut c = std::process::Command::new(&exe);
                c.arg("--worker").arg(&pp).arg(&lp2).arg(&hash);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    c.creation_flags(0x0800_0000);
                }
                if c.spawn().is_err() {
                    let _ = std::fs::write(&lp2, crate::tr!("ERREUR Impossible de lancer l'assistant : rien n'a été modifié.\nFIN ECHEC\n", "ERREUR Could not start the helper: nothing was changed.\nFIN ECHEC\n"));
                }
            });
            self.dialog = None;
            self.run = Some(Run { log: log_path, title, lines: Vec::new(), step: crate::tr!("Démarrage...", "Starting...").into(), pct: 0.0, done: None, queue, last_read: 0.0 });
            return;
        }
        // Started without administrator rights (e.g. from a terminal): ask for them for this run.
        let args = format!("--worker \"{}\" \"{}\" {}", plan_path.display(), log_path.display(), hash);
        let cmdline = format!("Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden", ops::ps_quote(&exe), ops::ps_quote(&args));
        std::thread::spawn(move || {
            let mut c = std::process::Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-Command", &cmdline]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x0800_0000);
            }
            let ok = c.status().map(|s| s.success()).unwrap_or(false);
            if !ok {
                let _ = std::fs::write(&lp, crate::tr!("ERREUR L'autorisation administrateur a été refusée : rien n'a été modifié.\nFIN ECHEC\n", "ERREUR Administrator permission was refused: nothing was changed.\nFIN ECHEC\n"));
            }
        });
        self.dialog = None;
        self.run = Some(Run { log: log_path, title, lines: Vec::new(), step: crate::tr!("Démarrage (autorisez DiskCraft à modifier l'appareil)...", "Starting (allow DiskCraft to make changes to your device)...").into(), pct: 0.0, done: None, queue, last_read: 0.0 });
    }

    fn poll_run(&mut self, now: f64) {
        let Some(r) = self.run.as_mut() else { return };
        if r.done.is_some() || now - r.last_read < 0.3 {
            return;
        }
        r.last_read = now;
        let Ok(text) = std::fs::read_to_string(&r.log) else { return };
        r.lines.clear();
        for l in text.lines() {
            if let Some(s) = l.strip_prefix("ETAPE ") {
                r.step = s.to_string();
                r.pct = 0.0;
            } else if let Some(p) = l.strip_prefix("PCT ") {
                r.pct = p.trim().parse().unwrap_or(r.pct);
            } else if let Some(s) = l.strip_prefix("INFO ") {
                r.lines.push(crate::i18n::log_line(s));
            } else if let Some(s) = l.strip_prefix("ERREUR ") {
                r.lines.push(format!("⚠ {}", crate::i18n::log_line(s)));
            } else if l.starts_with("FIN OK") {
                r.done = Some(true);
            } else if l.starts_with("FIN ECHEC") {
                r.done = Some(false);
            }
        }
        if r.done.is_some() && r.queue {
            self.reload();
        }
    }

    fn all_parts(&self) -> Vec<(u32, &Partition)> {
        self.preview.iter().flat_map(|d| d.parts.iter().filter(|p| p.kind != Kind::Extended).map(move |p| (d.number, p))).collect()
    }
    fn free_letters(&self) -> Vec<char> {
        let used: Vec<char> = self.all_parts().iter().filter_map(|(_, p)| p.letter).collect();
        ('D'..='Z').filter(|c| !used.contains(c)).collect()
    }
    fn disk(&self, n: u32) -> Option<&Disk> {
        self.preview.iter().find(|d| d.number == n)
    }
}

// ---------- small drawing helpers ----------

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD())
        .rounding(10.0)
        .stroke(Stroke::new(1.0_f32, LINE()))
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn primary(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).rounding(6.0)).clicked()
}

fn action(ui: &mut egui::Ui, icon: &str, text: &str, enabled: bool) -> bool {
    let w = ui.available_width();
    ui.add_enabled(enabled, egui::Button::new(RichText::new(format!("{icon}  {text}")).size(14.0)).min_size(Vec2::new(w, 32.0)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).rounding(6.0)).clicked()
}

fn heading(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.label(RichText::new(title).size(22.0).strong());
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(MUTED()));
    }
    ui.add_space(6.0);
}

fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::none().fill(color.gamma_multiply(0.15)).rounding(4.0).inner_margin(egui::Margin::symmetric(6.0, 1.0)).show(ui, |ui| {
        ui.label(RichText::new(text).size(11.5).color(color).strong());
    });
}

fn disk_combo(ui: &mut egui::Ui, id: &str, disks: &[Disk], value: &mut Option<u32>, filter: impl Fn(&Disk) -> bool) {
    let text = value
        .and_then(|n| disks.iter().find(|d| d.number == n))
        .map(|d| crate::trf!("Disque {} · {} · {}", "Disk {} · {} · {}", d.number, d.model, fmt_size(d.size)))
        .unwrap_or_else(|| crate::tr!("Choisir un disque...", "Choose a disk...").into());
    egui::ComboBox::from_id_salt(id).width(420.0).selected_text(text).show_ui(ui, |ui| {
        for d in disks.iter().filter(|d| filter(d)) {
            ui.selectable_value(value, Some(d.number), crate::trf!("Disque {} · {} · {} · {}", "Disk {} · {} · {} · {}", d.number, d.model, fmt_size(d.size), d.type_name()));
        }
    });
}

fn letter_combo(ui: &mut egui::Ui, id: &str, value: &mut Option<char>, choices: &[char], none_label: &str) {
    let text = value.map(|c| format!("{c}:")).unwrap_or_else(|| none_label.to_string());
    egui::ComboBox::from_id_salt(id).selected_text(text).show_ui(ui, |ui| {
        ui.selectable_value(value, None, none_label);
        for &c in choices {
            ui.selectable_value(value, Some(c), format!("{c}:"));
        }
    });
}

// ---------- main loop ----------

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        LIGHT.store(ctx.theme() == egui::Theme::Light, std::sync::atomic::Ordering::Relaxed);
        if self.started == 0.0 {
            self.started = now.max(0.001);
        }
        if let Some(rx) = &self.loading {
            if let Ok(res) = rx.try_recv() {
                self.loading = None;
                match res {
                    Ok(d) => {
                        self.set_disks(d);
                        self.load_note = None;
                    }
                    Err(e) => {
                        self.set_disks(crate::probe::demo_disks());
                        self.demo = true;
                        self.load_note = Some(crate::trf!("Les disques de ce PC n'ont pas pu être lus ({}) : disques d'exemple affichés.", "This PC's disks could not be read ({}): example disks are shown.", e.trim()));
                    }
                }
            }
        }
        if let Some(rx) = &self.health_loading {
            if let Ok(res) = rx.try_recv() {
                self.health_loading = None;
                if let Ok(h) = res {
                    self.health = h;
                } else if self.demo {
                    self.health = crate::probe::demo_health();
                }
            }
        }
        self.poll_run(now);

        let mut acts: Vec<Act> = Vec::new();
        self.nav(ctx, &mut acts);
        self.top(ctx, &mut acts);
        if !self.ops.is_empty() {
            self.queue_panel(ctx, &mut acts);
        }
        if self.page == Page::Partitions {
            self.side(ctx, &mut acts);
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG()).inner_margin(18.0)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.page {
                Page::Home => self.page_home(ui, &mut acts),
                Page::Health => self.page_health(ui),
                Page::Partitions => self.page_partitions(ui, &mut acts),
                Page::Clone => self.page_clone(ui, &mut acts),
                Page::Migrate => self.page_migrate(ui, &mut acts),
                Page::Convert => self.page_convert(ui, &mut acts),
                Page::WinPe => self.page_winpe(ui, &mut acts),
                Page::Support => self.page_support(ui),
                Page::Settings => self.page_settings(ui),
                Page::About => self.page_about(ui),
            });
        });
        self.dialogs(ctx, &mut acts);
        self.run_window(ctx, &mut acts);

        for a in acts {
            match a {
                Act::Select(s) => self.sel = s,
                Act::Open(d) => self.dialog = Some(d),
                Act::Add(op) => self.add_op(op),
                Act::Undo => {
                    self.ops.pop();
                    let _ = self.rebuild_preview();
                }
                Act::UndoAll => {
                    self.ops.clear();
                    let _ = self.rebuild_preview();
                }
                Act::Execute => match self.plan() {
                    Ok(steps) => {
                        let n = self.ops.len();
                        self.start_worker(steps, crate::trf!("Exécution de {n} opération(s)", "Running {n} operation(s)"), true)
                    }
                    Err(e) => self.dialog = Some(Dlg::Message { title: crate::tr!("Erreur", "Error").into(), text: e }),
                },
                Act::RunNow(steps, title) => self.start_worker(steps, title, false),
                Act::Reload => {
                    self.run = None;
                    self.reload();
                }
                Act::Page(p) => self.page = p,
            }
        }

        // --screenshot: wait for the disks, draw about 120 frames, then save the picture.
        if let Some(shot) = self.shot.as_mut() {
            shot.frames += 1;
            let ready = self.loading.is_none() || now - self.started > 12.0;
            if ready && shot.frames >= 120 && !shot.asked {
                shot.asked = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let mut saved = false;
            ctx.input(|i| {
                for e in &i.raw.events {
                    if let egui::Event::Screenshot { image, .. } = e {
                        let [w, h] = image.size;
                        let mut rgba = Vec::with_capacity(w * h * 4);
                        for p in &image.pixels {
                            rgba.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                        }
                        if let Some(parent) = std::path::Path::new(&shot.path).parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = image::save_buffer(&shot.path, &rgba, w as u32, h as u32, image::ColorType::Rgba8);
                        saved = true;
                    }
                }
            });
            if saved {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        } else if self.loading.is_some() || self.health_loading.is_some() || self.run.as_ref().is_some_and(|r| r.done.is_none()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl App {
    fn nav(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::SidePanel::left("nav").exact_width(236.0).resizable(false).frame(egui::Frame::none().fill(NAV()).stroke(Stroke::new(1.0_f32, LINE())).inner_margin(egui::Margin::symmetric(12.0, 14.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::new(38.0, 38.0)).rounding(8.0));
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new("DiskCraft").size(19.0).strong().color(TEXT()));
                    ui.label(RichText::new(crate::trf!("v{} · libre et gratuit", "v{} · free and open source", env!("CARGO_PKG_VERSION"))).size(11.0).color(MUTED()));
                });
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            let items = [
                (Page::Home, "🏠", crate::tr!("Accueil", "Home"), ICON_BLUE),
                (Page::Health, "♥", crate::tr!("Santé des disques", "Disk health"), ICON_GREEN),
                (Page::Partitions, "🖴", crate::tr!("Gestionnaire de partitions", "Partition manager"), ICON_TEAL),
                (Page::Clone, "🗐", crate::tr!("Cloner", "Clone"), ICON_PURPLE),
                (Page::Migrate, "➡", crate::tr!("Migrer Windows", "Migrate Windows"), ICON_GREEN),
                (Page::Convert, "⟲", crate::tr!("Convertisseurs", "Converters"), ICON_ORANGE),
                (Page::WinPe, "💿", crate::tr!("Disque de démarrage", "Boot disk"), ICON_TEAL),
                (Page::Support, "❓", crate::tr!("Aide & support", "Help & support"), ICON_RED),
                (Page::Settings, "⚙", crate::tr!("Paramètres", "Settings"), MUTED()),
                (Page::About, "ℹ", crate::tr!("À propos", "About"), MUTED()),
            ];
            for (page, icon, name, color) in items {
                let sel = self.page == page;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
                if sel || resp.hovered() {
                    ui.painter().rect_filled(rect, 7.0, if sel { NAV_HI() } else { NAV_HI().gamma_multiply(0.6) });
                }
                if sel {
                    ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 9.0), Vec2::new(3.0, 20.0)), 2.0, ACCENT);
                }
                let col = if sel { TEXT() } else { NAV_TEXT() };
                ui.painter().text(rect.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, icon, FontId::proportional(16.0), color);
                ui.painter().text(rect.left_center() + Vec2::new(42.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(14.0), col);
                if resp.clicked() {
                    acts.push(Act::Page(page));
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.label(RichText::new(tr!("Logiciel libre (GPL-3.0) · support gratuit", "Free software (GPL-3.0) · free support")).size(11.0).color(MUTED()));
            });
        });
    }

    fn top(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::TopBottomPanel::top("top").exact_height(54.0).frame(egui::Frame::none().fill(CARD()).stroke(Stroke::new(1.0_f32, LINE())).inner_margin(egui::Margin::symmetric(18.0, 10.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let title = match self.page {
                    Page::Home => crate::tr!("Accueil", "Home"),
                    Page::Health => crate::tr!("Santé des disques (SMART)", "Disk health (SMART)"),
                    Page::Partitions => crate::tr!("Gestionnaire de partitions", "Partition manager"),
                    Page::Clone => crate::tr!("Cloner un disque ou une partition", "Clone a disk or a partition"),
                    Page::Migrate => crate::tr!("Migrer Windows vers un SSD ou un disque dur", "Migrate Windows to an SSD or a hard drive"),
                    Page::Convert => crate::tr!("Convertisseurs", "Converters"),
                    Page::WinPe => crate::tr!("Disque de démarrage WinPE", "WinPE boot disk"),
                    Page::Support => crate::tr!("Aide & support gratuit", "Free help & support"),
                    Page::Settings => crate::tr!("Paramètres", "Settings"),
                    Page::About => crate::tr!("À propos de DiskCraft", "About DiskCraft"),
                };
                ui.label(RichText::new(title).size(17.0).strong());
                if self.loading.is_some() {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new(crate::tr!("Lecture des disques...", "Reading disks...")).color(MUTED()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let n = self.ops.len();
                    if primary(ui, &crate::trf!("▶  Exécuter {n} opération(s)", "▶  Execute {n} operation(s)"), n > 0) {
                        acts.push(Act::Open(Dlg::Confirm { backup_ok: false }));
                    }
                    if ui.add_enabled(n > 0, egui::Button::new(crate::tr!("↩ Annuler", "↩ Undo")).rounding(6.0)).clicked() {
                        acts.push(Act::Undo);
                    }
                    if ui.add_enabled(self.loading.is_none(), egui::Button::new(crate::tr!("⟳ Actualiser", "⟳ Refresh")).rounding(6.0)).clicked() {
                        acts.push(Act::Reload);
                    }
                });
            });
        });
    }

    fn queue_panel(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::TopBottomPanel::bottom("queue")
            .resizable(false)
            .frame(egui::Frame::none().fill(PENDING_BG()).stroke(Stroke::new(1.0_f32, PENDING.gamma_multiply(0.5))).inner_margin(egui::Margin::symmetric(18.0, 10.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::trf!("Opérations en attente ({})", "Pending operations ({})", self.ops.len())).strong().color(PENDING));
                    ui.label(RichText::new(crate::tr!("Rien n'est modifié tant que vous n'avez pas cliqué sur « Exécuter ». Vous pouvez annuler à tout moment.", "Nothing is changed until you click \"Execute\". You can undo at any time.")).color(MUTED()).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(crate::tr!("Tout annuler", "Undo all")).clicked() {
                            acts.push(Act::UndoAll);
                        }
                    });
                });
                for (i, op) in self.ops.iter().enumerate() {
                    ui.label(format!("{}. {}", i + 1, op.describe(&self.base)));
                }
            });
    }

    // ---------- partition manager ----------

    fn page_partitions(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        if let Some(n) = &self.load_note {
            egui::Frame::none().fill(INFO_BG()).rounding(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(n).color(ACCENT_DARK()));
            });
            ui.add_space(8.0);
        }
        let disks = self.preview.clone();
        for d in &disks {
            card(ui, |ui| {
                let header_sel = self.sel == Sel::Disk(d.number);
                let resp = ui
                    .horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::hover());
                        ui.painter().rect_filled(r, 7.0, if header_sel { ACCENT } else { CARD_HI() });
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "🖴", FontId::proportional(18.0), if header_sel { Color32::WHITE } else { MUTED() });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(crate::trf!("Disque {}", "Disk {}", d.number)).size(15.5).strong());
                                ui.label(RichText::new(&d.model).color(MUTED()));
                                if d.is_system || d.is_boot {
                                    badge(ui, "Windows", ACCENT);
                                }
                                if d.pending {
                                    badge(ui, crate::tr!("Modifié", "Modified"), PENDING);
                                }
                                if d.offline {
                                    badge(ui, crate::tr!("Hors ligne", "Offline"), DANGER);
                                }
                            });
                            let bus = if d.bus.is_empty() { "—".to_string() } else { d.bus.clone() };
                            ui.label(RichText::new(format!("{} · {} · {}", fmt_size(d.size), d.type_name(), bus)).size(12.0).color(MUTED()));
                        });
                    })
                    .response
                    .interact(Sense::click());
                if resp.clicked() {
                    acts.push(Act::Select(Sel::Disk(d.number)));
                }
                ui.add_space(6.0);
                self.disk_bar(ui, d, acts);
                ui.add_space(6.0);
                self.part_table(ui, d, acts);
            });
            ui.add_space(12.0);
        }
        if disks.is_empty() && self.loading.is_none() {
            ui.label(crate::tr!("Aucun disque.", "No disks."));
        }
    }

    fn disk_bar(&self, ui: &mut egui::Ui, d: &Disk, acts: &mut Vec<Act>) {
        let segs = d.segments();
        let width = ui.available_width();
        let h = 58.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
        if segs.is_empty() {
            ui.painter().rect_filled(rect, 6.0, CARD_HI());
            let msg = if d.size == 0 { crate::tr!("Aucun support inséré", "No media inserted") } else { crate::tr!("Disque non initialisé : sélectionnez le disque puis « Initialiser en GPT » ou « en MBR »", "Uninitialized disk: select the disk, then \"Initialize as GPT\" or \"as MBR\"") };
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(13.0), MUTED());
            return;
        }
        let sizes: Vec<u64> = segs
            .iter()
            .map(|s| match s {
                Segment::Part(i) => d.parts[*i].size,
                Segment::Free { size, .. } => *size,
            })
            .collect();
        let total: f64 = sizes.iter().sum::<u64>().max(1) as f64;
        let min_w = 44.0f32;
        let gap = 3.0f32;
        let avail = (width - gap * segs.len().saturating_sub(1) as f32).max(10.0);
        let raw: Vec<f32> = sizes.iter().map(|&s| (s as f64 / total) as f32 * avail).collect();
        let small: f32 = raw.iter().filter(|&&w| w < min_w).count() as f32 * min_w;
        let big: f32 = raw.iter().filter(|&&w| w >= min_w).sum();
        let scale = if big > 0.0 { (avail - small).max(10.0) / big } else { 1.0 };
        let mut x = rect.left();
        for (k, s) in segs.iter().enumerate() {
            let w = if raw[k] < min_w { min_w } else { raw[k] * scale };
            let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(w, h));
            x += w + gap;
            let resp = ui.interact(r, ui.id().with(("seg", d.number, k)), Sense::click());
            let p = ui.painter().with_clip_rect(r);
            let (sel, label, sub, color, used_frac, pending) = match s {
                Segment::Part(i) => {
                    let pt = &d.parts[*i];
                    let frac = pt.used.map(|u| u as f32 / pt.size.max(1) as f32).unwrap_or(0.0);
                    (self.sel == Sel::Part(d.number, pt.offset), pt.title(), format!("{} {}", fmt_size(pt.size), pt.fs.name()), fs_color(pt), frac, pt.pending)
                }
                Segment::Free { offset, size } => (
                    matches!(self.sel, Sel::Free(n, o, _) if n == d.number && o == *offset),
                    crate::tr!("Non alloué", "Unallocated").to_string(),
                    fmt_size(*size),
                    Color32::from_rgb(0x7d, 0x83, 0x8e),
                    0.0,
                    false,
                ),
            };
            p.rect_filled(r, 6.0, color.gamma_multiply(0.14));
            p.rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(r.width(), 5.0)), Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 }, color);
            if used_frac > 0.0 {
                p.rect_filled(egui::Rect::from_min_size(r.min + Vec2::new(0.0, 5.0), Vec2::new(r.width() * used_frac.min(1.0), h - 5.0)), 0.0, color.gamma_multiply(0.42));
            }
            if matches!(s, Segment::Free { .. }) {
                let mut xx = r.left() - h;
                while xx < r.right() {
                    p.line_segment([egui::pos2(xx, r.bottom()), egui::pos2(xx + h, r.top() + 5.0)], Stroke::new(1.0_f32, HATCH()));
                    xx += 9.0;
                }
            }
            p.text(r.left_top() + Vec2::new(7.0, 13.0), Align2::LEFT_TOP, &label, FontId::proportional(13.0), TEXT());
            p.text(r.left_top() + Vec2::new(7.0, 32.0), Align2::LEFT_TOP, &sub, FontId::proportional(11.5), MUTED());
            let stroke = if sel {
                Stroke::new(2.5_f32, ACCENT_DARK())
            } else if pending {
                Stroke::new(2.0_f32, PENDING)
            } else if resp.hovered() {
                Stroke::new(1.5_f32, color)
            } else {
                Stroke::new(1.0_f32, color.gamma_multiply(0.4))
            };
            ui.painter().rect_stroke(r, 6.0, stroke);
            if resp.clicked() {
                acts.push(Act::Select(match s {
                    Segment::Part(i) => Sel::Part(d.number, d.parts[*i].offset),
                    Segment::Free { offset, size } => Sel::Free(d.number, *offset, *size),
                }));
            }
            resp.on_hover_text(match s {
                Segment::Part(i) => {
                    let pt = &d.parts[*i];
                    crate::trf!("{}\n{} · {}\nUtilisé : {}", "{}\n{} · {}\nUsed: {}", pt.title(), pt.kind.name(), fmt_size(pt.size), pt.used.map(fmt_size).unwrap_or_else(|| "—".into()))
                }
                Segment::Free { size, .. } => crate::trf!("Espace non alloué : {}", "Unallocated space: {}", fmt_size(*size)),
            });
        }
    }

    fn part_table(&self, ui: &mut egui::Ui, d: &Disk, acts: &mut Vec<Act>) {
        if d.segments().is_empty() {
            return;
        }
        egui::Grid::new(("grid", d.number)).num_columns(7).spacing([18.0, 4.0]).striped(true).min_col_width(60.0).show(ui, |ui| {
            for h in ["Partition", crate::tr!("Système de fichiers", "File system"), crate::tr!("Taille", "Size"), crate::tr!("Utilisé", "Used"), crate::tr!("Libre", "Free"), crate::tr!("Type", "Type"), crate::tr!("État", "Status")] {
                ui.label(RichText::new(h).size(12.0).color(MUTED()).strong());
            }
            ui.end_row();
            for s in d.segments() {
                match s {
                    Segment::Part(i) => {
                        let p = &d.parts[i];
                        let sel = self.sel == Sel::Part(d.number, p.offset);
                        if ui.selectable_label(sel, p.title()).clicked() {
                            acts.push(Act::Select(Sel::Part(d.number, p.offset)));
                        }
                        ui.label(p.fs.name());
                        ui.label(fmt_size(p.size));
                        ui.label(p.used.map(fmt_size).unwrap_or_else(|| "—".into()));
                        ui.label(p.free().map(fmt_size).unwrap_or_else(|| "—".into()));
                        ui.label(p.kind.name());
                        let mut st = Vec::new();
                        if p.is_boot {
                            st.push(crate::tr!("Démarrage", "Boot"));
                        }
                        if p.is_system {
                            st.push(crate::tr!("Système", "System"));
                        }
                        if p.is_active {
                            st.push(crate::tr!("Active", "Active"));
                        }
                        if p.letter.is_none() && p.kind == Kind::Data {
                            st.push(crate::tr!("Masquée", "Hidden"));
                        }
                        if p.pending {
                            ui.label(RichText::new(crate::tr!("En attente", "Pending")).color(PENDING));
                        } else {
                            ui.label(if st.is_empty() { crate::tr!("Aucun", "None").to_string() } else { st.join(", ") });
                        }
                    }
                    Segment::Free { offset, size } => {
                        if ui.selectable_label(matches!(self.sel, Sel::Free(n, o, _) if n == d.number && o == offset), RichText::new(crate::tr!("Non alloué", "Unallocated")).italics()).clicked() {
                            acts.push(Act::Select(Sel::Free(d.number, offset, size)));
                        }
                        ui.label("—");
                        ui.label(fmt_size(size));
                        for _ in 0..4 {
                            ui.label("—");
                        }
                    }
                }
                ui.end_row();
            }
        });
    }

    fn side(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::SidePanel::right("side").exact_width(270.0).resizable(false).frame(egui::Frame::none().fill(CARD()).stroke(Stroke::new(1.0_f32, LINE())).inner_margin(14.0)).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.sel {
                Sel::None => {
                    ui.label(RichText::new(crate::tr!("Sélectionnez une partition", "Select a partition")).size(15.0).strong());
                    ui.label(RichText::new(crate::tr!("Cliquez sur une partition, un espace non alloué ou le titre d'un disque pour voir ce que vous pouvez faire.", "Click a partition, some unallocated space or a disk's title to see what you can do.")).color(MUTED()));
                    ui.add_space(10.0);
                    let d = self.preview.len();
                    let p = self.all_parts().len();
                    ui.label(crate::trf!("{d} disque(s), {p} partition(s)", "{d} disk(s), {p} partition(s)"));
                    ui.add_space(10.0);
                    ui.label(RichText::new(crate::tr!("Légende", "Legend")).strong());
                    for (name, c) in [
                        ("NTFS", Color32::from_rgb(0x2f, 0x7f, 0xd8)),
                        ("FAT32", Color32::from_rgb(0x3f, 0xa6, 0x5a)),
                        ("exFAT", Color32::from_rgb(0x1f, 0xa3, 0xa8)),
                        (crate::tr!("Système EFI", "EFI system"), Color32::from_rgb(0x8a, 0x63, 0xd2)),
                        (crate::tr!("Récupération", "Recovery"), Color32::from_rgb(0xd9, 0x8b, 0x2b)),
                        (crate::tr!("Non alloué", "Unallocated"), Color32::from_rgb(0xb4, 0xbb, 0xc6)),
                    ] {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(r, 3.0, c);
                            ui.label(name);
                        });
                    }
                }
                Sel::Part(n, o) => {
                    let Some(d) = self.disk(n).cloned() else { return };
                    let Some(i) = d.part_at(o) else { return };
                    let p = d.parts[i].clone();
                    ui.label(RichText::new(p.title()).size(17.0).strong());
                    ui.label(RichText::new(crate::trf!("Disque {} · {} · {}", "Disk {} · {} · {}", n, p.fs.name(), fmt_size(p.size))).color(MUTED()));
                    if let Some(u) = p.used {
                        ui.add(egui::ProgressBar::new(u as f32 / p.size.max(1) as f32).text(crate::trf!("{} utilisés sur {}", "{} used of {}", fmt_size(u), fmt_size(p.size))));
                    }
                    ui.add_space(8.0);
                    let prot = p.protected();
                    let ntfs = p.fs == Fs::Ntfs;
                    let can_resize = matches!(p.fs, Fs::Ntfs | Fs::Refs);
                    let can_move = !prot && !matches!(p.kind, Kind::Logical | Kind::Extended | Kind::LdmData | Kind::LdmMeta);
                    if action(ui, "↔", crate::tr!("Redimensionner / déplacer", "Resize / move"), can_resize || can_move) {
                        let lo = o - d.free_before(o);
                        let hi = p.end() + d.free_after(o);
                        // Its own place plus every other free area of the disk it could be dropped into.
                        let mut regions = vec![(lo, hi)];
                        if can_move {
                            for s in d.segments() {
                                if let Segment::Free { offset: fo, size: fs } = s {
                                    if (fo + fs <= lo || fo >= hi) && fs >= MIB {
                                        regions.push((fo, fo + fs));
                                    }
                                }
                            }
                        }
                        acts.push(Act::Open(Dlg::Resize {
                            disk: n,
                            offset: o,
                            pos: o,
                            size_mb: (p.size / MIB) as f64,
                            regions,
                            min: ops::min_size(&p),
                            cur: p.size,
                            can_resize,
                            can_move,
                            grab: None,
                        }));
                    }
                    if action(ui, "🔗", crate::tr!("Fusionner avec la suivante", "Merge with the next one"), ntfs && d.next_part(o).is_some()) {
                        acts.push(Act::Open(Dlg::Merge { disk: n, first: o }));
                    }
                    if action(ui, "🖹", crate::tr!("Formater", "Format"), !prot) {
                        acts.push(Act::Open(Dlg::Format { disk: n, offset: o, fs: if p.fs == Fs::Raw { Fs::Ntfs } else { p.fs.clone() }, label: p.label.clone(), cluster: 0, quick: true }));
                    }
                    if action(ui, "🗑", crate::tr!("Supprimer", "Delete"), !prot) {
                        acts.push(Act::Open(Dlg::Delete { disk: n, offset: o }));
                    }
                    if action(ui, "✖", crate::tr!("Effacer les données", "Wipe data"), !prot) {
                        acts.push(Act::Open(Dlg::Wipe { disk: n, offset: Some(o), passes: 1 }));
                    }
                    if action(ui, "✔", crate::tr!("Vérifier / test de surface", "Check / surface test"), true) {
                        acts.push(Act::Open(Dlg::Check { disk: n, offset: o, mode: 0 }));
                    }
                    if action(ui, "🗐", crate::tr!("Cloner la partition", "Clone partition"), true) {
                        self.clone_part_mode = true;
                        self.clone_psrc = Some((n, o));
                        acts.push(Act::Page(Page::Clone));
                    }
                    if action(ui, "⟲", crate::tr!("Convertir NTFS en FAT32", "Convert NTFS to FAT32"), ntfs && !prot) {
                        self.fat_part = Some((n, o));
                        acts.push(Act::Page(Page::Convert));
                    }
                    ui.separator();
                    if action(ui, "🔤", crate::tr!("Changer la lettre / masquer", "Change letter / hide"), !p.is_boot) {
                        acts.push(Act::Open(Dlg::Letter { disk: n, offset: o, letter: p.letter }));
                    }
                    if action(ui, "🏷", crate::tr!("Changer le nom", "Change label"), p.fs != Fs::Raw) {
                        acts.push(Act::Open(Dlg::Label { disk: n, offset: o, label: p.label.clone() }));
                    }
                    if action(ui, "★", crate::tr!("Rendre active", "Set active"), d.style == Style::Mbr && !p.is_active) {
                        acts.push(Act::Add(Op::SetActive { disk: n, offset: o }));
                    }
                    if prot {
                        ui.add_space(6.0);
                        ui.label(RichText::new(crate::tr!("Windows utilise cette partition : la supprimer, la formater ou l'effacer est bloqué.", "Windows is using this partition: deleting, formatting or wiping it is blocked.")).size(12.0).color(MUTED()));
                    }
                }
                Sel::Free(n, o, s) => {
                    ui.label(RichText::new(crate::tr!("Espace non alloué", "Unallocated space")).size(17.0).strong());
                    ui.label(RichText::new(crate::trf!("Disque {n} · {}", "Disk {n} · {}", fmt_size(s))).color(MUTED()));
                    ui.add_space(8.0);
                    if action(ui, "＋", crate::tr!("Créer une partition", "Create a partition"), true) {
                        let letter = self.free_letters().first().copied();
                        acts.push(Act::Open(Dlg::Create { disk: n, offset: o, max: s, size_mb: (s / MIB) as f64, fs: Fs::Ntfs, label: String::new(), letter }));
                    }
                    if action(ui, "🗐", crate::tr!("Cloner une partition ici", "Clone a partition here"), true) {
                        self.clone_part_mode = true;
                        self.clone_pdst = Some((n, o));
                        acts.push(Act::Page(Page::Clone));
                    }
                    ui.label(RichText::new(crate::tr!("Pour agrandir la partition placée juste avant, sélectionnez-la puis « Redimensionner ».", "To extend the partition just before it, select that partition, then \"Resize\".")).size(12.0).color(MUTED()));
                }
                Sel::Disk(n) => {
                    let Some(d) = self.disk(n).cloned() else { return };
                    ui.label(RichText::new(crate::trf!("Disque {n}", "Disk {n}")).size(17.0).strong());
                    ui.label(RichText::new(format!("{} · {} · {}", d.model, fmt_size(d.size), d.type_name())).color(MUTED()));
                    ui.add_space(8.0);
                    let sys = d.is_system || d.is_boot;
                    if action(ui, "🗐", crate::tr!("Cloner le disque", "Clone disk"), true) {
                        self.clone_part_mode = false;
                        self.clone_src = Some(n);
                        acts.push(Act::Page(Page::Clone));
                    }
                    if action(ui, "➡", crate::tr!("Migrer Windows vers un autre disque", "Migrate Windows to another disk"), sys) {
                        acts.push(Act::Page(Page::Migrate));
                    }
                    if action(ui, "⟲", if d.style == Style::Raw { crate::tr!("Initialiser en GPT", "Initialize as GPT") } else { crate::tr!("Convertir en GPT", "Convert to GPT") }, d.style != Style::Gpt) {
                        acts.push(Act::Add(Op::ToGpt { disk: n }));
                    }
                    if action(ui, "⟲", if d.style == Style::Raw { crate::tr!("Initialiser en MBR", "Initialize as MBR") } else { crate::tr!("Convertir en MBR", "Convert to MBR") }, d.style != Style::Mbr && !sys) {
                        acts.push(Act::Add(Op::ToMbr { disk: n }));
                    }
                    if action(ui, "⟲", crate::tr!("Convertir en disque de base", "Convert to basic disk"), d.dynamic) {
                        self.dyn_disk = Some(n);
                        acts.push(Act::Page(Page::Convert));
                    }
                    if action(ui, "✖", crate::tr!("Effacer tout le disque", "Wipe the whole disk"), !sys) {
                        acts.push(Act::Open(Dlg::Wipe { disk: n, offset: None, passes: 1 }));
                    }
                }
            });
        });
    }

    // ---------- other pages ----------

    fn page_home(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::new(64.0, 64.0)).rounding(14.0));
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new(crate::tr!("Bienvenue dans DiskCraft", "Welcome to DiskCraft")).size(27.0).strong());
                ui.label(RichText::new(crate::tr!("Un gestionnaire de disques open source — local, gratuit et complet.", "An open-source disk manager — local, free and complete.")).size(14.5).color(MUTED()));
            });
        });
        ui.add_space(14.0);
        if let Some(n) = &self.load_note {
            egui::Frame::none().fill(INFO_BG()).rounding(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(n).color(ACCENT_DARK()));
            });
            ui.add_space(10.0);
        }
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("👥").size(20.0).color(ICON_BLUE));
                ui.vertical(|ui| {
                    ui.label(RichText::new(crate::tr!("Rejoindre la communauté DiskCraft", "Join the DiskCraft community")).size(15.0).strong());
                    ui.label(RichText::new(crate::tr!("Obtenez de l'aide gratuitement, proposez des idées et suivez le développement.", "Get free help, suggest ideas and follow development.")).color(MUTED()));
                });
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if primary(ui, crate::tr!("💬  Poser une question", "💬  Ask a question"), true) {
                    open_url("https://github.com/Wikiabo-Labs/CraftDisk/discussions");
                }
                if ui.add(egui::Button::new(crate::tr!("🐛  Signaler un problème", "🐛  Report a problem")).rounding(14.0)).clicked() {
                    open_url("https://github.com/Wikiabo-Labs/CraftDisk/issues");
                }
                if ui.add(egui::Button::new(crate::tr!("</>  DiskCraft sur GitHub", "</>  DiskCraft on GitHub")).rounding(14.0)).clicked() {
                    open_url("https://github.com/Wikiabo-Labs/CraftDisk");
                }
                if ui.add(egui::Button::new(crate::tr!("📖  Guide d'utilisation", "📖  User guide")).rounding(14.0)).clicked() {
                    acts.push(Act::Page(Page::Support));
                }
            });
        });
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Outils recommandés", "Recommended tools")).size(15.0).strong());
            ui.add_space(6.0);
            let tools = [
                ("↔", ICON_BLUE, crate::tr!("Redimensionner / déplacer", "Resize / move"), crate::tr!("Agrandir C: · Réduire · Déplacer une partition", "Extend C: · Shrink · Move a partition"), Page::Partitions),
                ("🗐", ICON_PURPLE, crate::tr!("Cloner", "Clone"), crate::tr!("Disque entier · Partition · Copie à chaud", "Whole disk · Partition · Live copy"), Page::Clone),
                ("➡", ICON_GREEN, crate::tr!("Migrer Windows", "Migrate Windows"), crate::tr!("Vers un SSD ou un nouveau disque · Démarrage réparé", "To an SSD or a new disk · Boot repaired"), Page::Migrate),
                ("⟲", ICON_ORANGE, crate::tr!("Convertir", "Convert"), crate::tr!("MBR / GPT · NTFS en FAT32 · Dynamique en base", "MBR / GPT · NTFS to FAT32 · Dynamic to basic"), Page::Convert),
                ("💿", ICON_TEAL, crate::tr!("Disque de démarrage", "Boot disk"), crate::tr!("Clé USB ou ISO WinPE avec DiskCraft", "WinPE USB stick or ISO with DiskCraft"), Page::WinPe),
                ("✔", ICON_RED, crate::tr!("Vérifier un disque", "Check a disk"), crate::tr!("Erreurs du système de fichiers · Test de surface", "File system errors · Surface test"), Page::Partitions),
            ];
            let w = ((ui.available_width() - 2.0 * 10.0) / 3.0).max(200.0);
            egui::Grid::new("tools").num_columns(3).spacing([10.0, 10.0]).show(ui, |ui| {
                for (k, (icon, color, title, sub, page)) in tools.iter().enumerate() {
                    let resp = egui::Frame::none()
                        .fill(CARD_HI())
                        .rounding(8.0)
                        .stroke(Stroke::new(1.0_f32, LINE()))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(w - 26.0);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(*icon).size(18.0).color(*color));
                                    ui.label(RichText::new(*title).size(14.5).strong());
                                });
                                ui.add(egui::Label::new(RichText::new(*sub).size(12.0).color(MUTED())).truncate());
                                ui.label(RichText::new(crate::tr!("Utiliser", "Open")).size(12.5).strong().color(ACCENT_DARK()));
                            });
                        })
                        .response
                        .interact(Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        acts.push(Act::Page(*page));
                    }
                    if k % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(crate::tr!("Vos disques", "Your disks")).size(17.0).strong());
            if self.loading.is_some() {
                ui.add(egui::Spinner::new());
            }
        });
        ui.add_space(4.0);
        let disks = self.preview.clone();
        for d in &disks {
            let used: u64 = d.parts.iter().filter_map(|p| p.used).sum();
            let allocated: u64 = d.parts.iter().filter(|p| p.kind != Kind::Extended).map(|p| p.size).sum();
            let resp = egui::Frame::none()
                .fill(CARD())
                .rounding(8.0)
                .stroke(Stroke::new(1.0_f32, LINE()))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🖴").size(20.0).color(if d.is_system || d.is_boot { ICON_GREEN } else { ICON_BLUE }));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(crate::trf!("Disque {}", "Disk {}", d.number)).strong());
                                ui.label(RichText::new(&d.model).color(MUTED()));
                                if d.is_system || d.is_boot {
                                    badge(ui, "Windows", ICON_GREEN);
                                }
                            });
                            ui.label(RichText::new(format!("{} · {} · {} partition(s)", fmt_size(d.size), d.type_name(), d.parts.len())).size(12.0).color(MUTED()));
                            if d.size > 0 {
                                let frac = used as f32 / d.size as f32;
                                ui.add(egui::ProgressBar::new(frac).desired_width(380.0).text(crate::trf!("{} utilisés · {} non alloués", "{} used · {} unallocated", fmt_size(used), fmt_size(d.size.saturating_sub(allocated)))));
                            } else {
                                ui.label(RichText::new(crate::tr!("Aucun support inséré", "No media inserted")).size(12.0).color(MUTED()));
                            }
                        });
                    });
                })
                .response
                .interact(Sense::click());
            if resp.clicked() {
                acts.push(Act::Select(Sel::Disk(d.number)));
                acts.push(Act::Page(Page::Partitions));
            }
            ui.add_space(6.0);
        }
    }

    fn page_clone(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, crate::tr!("Cloner", "Clone"), crate::tr!("Copie exacte secteur par secteur. Les volumes en cours d'utilisation sont copiés depuis un instantané, sans redémarrer.", "An exact sector-by-sector copy. Volumes in use are copied from a snapshot, without restarting."));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.clone_part_mode, false, RichText::new(crate::tr!("  Cloner un disque  ", "  Clone a disk  ")).size(14.0));
            ui.selectable_value(&mut self.clone_part_mode, true, RichText::new(crate::tr!("  Cloner une partition  ", "  Clone a partition  ")).size(14.0));
        });
        ui.add_space(8.0);
        let disks = self.preview.clone();
        if !self.clone_part_mode {
            card(ui, |ui| {
                ui.label(RichText::new(crate::tr!("1. Disque source", "1. Source disk")).strong());
                disk_combo(ui, "csrc", &disks, &mut self.clone_src, |_| true);
                ui.add_space(6.0);
                ui.label(RichText::new(crate::tr!("2. Disque cible (il sera entièrement remplacé)", "2. Target disk (it will be completely replaced)")).strong());
                let src = self.clone_src;
                disk_combo(ui, "cdst", &disks, &mut self.clone_dst, |d| Some(d.number) != src && !d.is_system && !d.is_boot);
                ui.checkbox(&mut self.fit, crate::tr!("Agrandir la dernière partition pour remplir le disque cible", "Extend the last partition to fill the target disk"));
                if let (Some(s), Some(t)) = (self.clone_src.and_then(|n| disks.iter().find(|d| d.number == n)), self.clone_dst.and_then(|n| disks.iter().find(|d| d.number == n))) {
                    ui.add_space(6.0);
                    ui.label(crate::trf!("À copier : {}, place disponible : {}", "To copy: {}, space available: {}", fmt_size(s.used_extent()), fmt_size(t.size)));
                    if !t.parts.is_empty() {
                        ui.label(RichText::new(crate::trf!("⚠ Les {} partition(s) du disque {} seront effacées.", "⚠ The {} partition(s) on disk {} will be erased.", t.parts.len(), t.number)).color(DANGER));
                    }
                }
                ui.add_space(6.0);
                if primary(ui, crate::tr!("Ajouter à la file d'attente", "Add to the queue"), self.clone_src.is_some() && self.clone_dst.is_some()) {
                    acts.push(Act::Add(Op::CloneDisk { src: self.clone_src.unwrap(), dst: self.clone_dst.unwrap(), fit: self.fit }));
                    acts.push(Act::Page(Page::Partitions));
                }
            });
        } else {
            let parts: Vec<(u32, Partition)> = self.all_parts().into_iter().map(|(n, p)| (n, p.clone())).collect();
            card(ui, |ui| {
                ui.label(RichText::new(crate::tr!("1. Partition source", "1. Source partition")).strong());
                let txt = self
                    .clone_psrc
                    .and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o))
                    .map(|(n, p)| crate::trf!("Disque {n} · {} · {}", "Disk {n} · {} · {}", p.title(), fmt_size(p.size)))
                    .unwrap_or_else(|| crate::tr!("Choisir une partition...", "Choose a partition...").into());
                egui::ComboBox::from_id_salt("psrc").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                    for (n, p) in &parts {
                        ui.selectable_value(&mut self.clone_psrc, Some((*n, p.offset)), crate::trf!("Disque {n} · {} · {} · {}", "Disk {n} · {} · {} · {}", p.title(), p.fs.name(), fmt_size(p.size)));
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new(crate::tr!("2. Espace non alloué cible", "2. Target unallocated space")).strong());
                let need = self.clone_psrc.and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o)).map(|(_, p)| p.size).unwrap_or(0);
                let frees: Vec<(u32, u64, u64)> = disks
                    .iter()
                    .flat_map(|d| d.segments().into_iter().filter_map(move |s| if let Segment::Free { offset, size } = s { Some((d.number, offset, size)) } else { None }))
                    .collect();
                let txt = self
                    .clone_pdst
                    .and_then(|(n, o)| frees.iter().find(|f| f.0 == n && f.1 == o))
                    .map(|f| crate::trf!("Disque {} · non alloué · {}", "Disk {} · unallocated · {}", f.0, fmt_size(f.2)))
                    .unwrap_or_else(|| crate::tr!("Choisir un espace libre...", "Choose some free space...").into());
                egui::ComboBox::from_id_salt("pdst").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                    for f in &frees {
                        let ok = f.2 >= need;
                        ui.add_enabled_ui(ok, |ui| {
                            ui.selectable_value(&mut self.clone_pdst, Some((f.0, f.1)), crate::trf!("Disque {} · non alloué · {}{}", "Disk {} · unallocated · {}{}", f.0, fmt_size(f.2), if ok { "" } else { crate::tr!(" (trop petit)", " (too small)") }));
                        });
                    }
                });
                if frees.is_empty() {
                    ui.label(RichText::new(crate::tr!("Aucun espace non alloué : réduisez ou supprimez d'abord une partition.", "No unallocated space: shrink or delete a partition first.")).color(MUTED()));
                }
                ui.add_space(6.0);
                if primary(ui, crate::tr!("Ajouter à la file d'attente", "Add to the queue"), self.clone_psrc.is_some() && self.clone_pdst.is_some()) {
                    let (sn, so) = self.clone_psrc.unwrap();
                    let (dn, dof) = self.clone_pdst.unwrap();
                    acts.push(Act::Add(Op::ClonePart { src_disk: sn, src_offset: so, dst_disk: dn, dst_offset: dof }));
                    acts.push(Act::Page(Page::Partitions));
                }
            });
        }
    }

    fn page_migrate(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, crate::tr!("Migrer Windows", "Migrate Windows"), crate::tr!("Déplacez Windows, vos programmes et vos fichiers vers un SSD ou un nouveau disque, puis démarrez dessus.", "Move Windows, your programs and your files to an SSD or a new disk, then start the PC from it."));
        let disks = self.preview.clone();
        let Some(sys) = disks.iter().find(|d| d.is_system || d.is_boot) else {
            card(ui, |ui| {
                ui.label(crate::tr!("Le disque de Windows n'a pas été trouvé.", "The Windows disk was not found."));
            });
            return;
        };
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Disque actuel de Windows", "Current Windows disk")).strong());
            ui.label(crate::trf!("Disque {} · {} · {} · {} à copier", "Disk {} · {} · {} · {} to copy", sys.number, sys.model, sys.type_name(), fmt_size(sys.used_extent())));
        });
        ui.add_space(10.0);
        ui.label(RichText::new(crate::tr!("Choisissez le disque cible", "Choose the target disk")).size(15.0).strong());
        for d in disks.iter().filter(|d| d.number != sys.number) {
            let fits = d.size >= sys.used_extent() + gpt_tail_sectors(sys.sector) * sys.sector;
            let sel = self.mig_dst == Some(d.number);
            let resp = egui::Frame::none()
                .fill(if sel { ACCENT.gamma_multiply(0.08) } else { CARD() })
                .rounding(10.0)
                .stroke(Stroke::new(if sel { 2.0_f32 } else { 1.0 }, if sel { ACCENT } else { LINE() }))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(crate::trf!("Disque {}", "Disk {}", d.number)).strong());
                        ui.label(&d.model);
                        ui.label(RichText::new(fmt_size(d.size)).color(MUTED()));
                        if d.bus.contains("NVMe") || d.model.to_lowercase().contains("ssd") {
                            badge(ui, "SSD", ACCENT);
                        }
                        if !fits {
                            badge(ui, crate::tr!("Trop petit", "Too small"), DANGER);
                        }
                    });
                })
                .response
                .interact(Sense::click());
            if resp.clicked() && fits {
                self.mig_dst = Some(d.number);
            }
            ui.add_space(4.0);
        }
        ui.add_space(6.0);
        ui.checkbox(&mut self.fit, crate::tr!("Utiliser tout l'espace du disque cible (agrandir la dernière partition)", "Use all the space of the target disk (extend the last partition)"));
        ui.label(RichText::new(crate::tr!("Après la copie, DiskCraft reconstruit le démarrage du nouveau disque : il suffit ensuite de le choisir dans le menu de démarrage du PC.", "After the copy, DiskCraft rebuilds the new disk's boot files: then just pick it in the PC's boot menu.")).color(MUTED()));
        if let Some(t) = self.mig_dst {
            ui.label(RichText::new(crate::trf!("⚠ Tout le contenu du disque {t} sera remplacé.", "⚠ Everything on disk {t} will be replaced.")).color(DANGER));
        }
        if primary(ui, crate::tr!("Ajouter la migration à la file d'attente", "Add the migration to the queue"), self.mig_dst.is_some()) {
            acts.push(Act::Add(Op::MigrateOs { src: sys.number, dst: self.mig_dst.unwrap(), fit: self.fit }));
            acts.push(Act::Page(Page::Partitions));
        }
    }

    fn page_convert(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, crate::tr!("Convertisseurs", "Converters"), crate::tr!("Changez le type d'un disque ou d'une partition sans perdre vos données.", "Change the type of a disk or partition without losing your data."));
        let disks = self.preview.clone();
        card(ui, |ui| {
            ui.label(RichText::new("MBR / GPT").size(15.0).strong());
            ui.label(RichText::new(crate::tr!("GPT est nécessaire pour les disques de plus de 2 To et le démarrage UEFI ; MBR pour les vieux PC en BIOS.", "GPT is needed for disks larger than 2 TB and for UEFI boot; MBR for old BIOS PCs.")).color(MUTED()));
            disk_combo(ui, "conv", &disks, &mut self.conv_disk, |_| true);
            if let Some(d) = self.conv_disk.and_then(|n| disks.iter().find(|d| d.number == n)) {
                let n = d.number;
                ui.label(crate::trf!("Type actuel : {}", "Current type: {}", d.type_name()));
                ui.horizontal(|ui| {
                    if primary(ui, crate::tr!("Convertir en GPT", "Convert to GPT"), d.style != Style::Gpt) {
                        acts.push(Act::Add(Op::ToGpt { disk: n }));
                    }
                    if primary(ui, crate::tr!("Convertir en MBR", "Convert to MBR"), d.style != Style::Mbr) {
                        acts.push(Act::Add(Op::ToMbr { disk: n }));
                    }
                });
                if d.is_system || d.is_boot {
                    ui.label(RichText::new(crate::tr!("Disque de Windows : la conversion en GPT utilise l'outil mbr2gpt de Windows ; passez ensuite le PC en mode UEFI.", "Windows disk: the GPT conversion uses Windows' mbr2gpt tool; then switch the PC to UEFI mode.")).size(12.0).color(MUTED()));
                }
            }
        });
        ui.add_space(10.0);
        let letters: Vec<(char, u64)> = self.all_parts().iter().filter_map(|(_, p)| Some((p.letter?, p.free()?))).collect();
        let temp_choices: Vec<char> = letters.iter().map(|l| l.0).collect();
        let parts: Vec<(u32, Partition)> = self.all_parts().into_iter().filter(|(_, p)| p.fs == Fs::Ntfs && !p.protected()).map(|(n, p)| (n, p.clone())).collect();
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("NTFS en FAT32", "NTFS to FAT32")).size(15.0).strong());
            ui.label(RichText::new(crate::tr!("FAT32 se lit partout (TV, consoles, voitures, Mac). Limites : fichiers de 4 Go maximum, partition de 2 To maximum.", "FAT32 can be read everywhere (TVs, consoles, cars, Macs). Limits: files up to 4 GB, partitions up to 2 TB.")).color(MUTED()));
            let txt = self
                .fat_part
                .and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o))
                .map(|(n, p)| crate::trf!("Disque {n} · {} · {}", "Disk {n} · {} · {}", p.title(), fmt_size(p.size)))
                .unwrap_or_else(|| crate::tr!("Choisir une partition NTFS...", "Choose an NTFS partition...").into());
            egui::ComboBox::from_id_salt("fat").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                for (n, p) in &parts {
                    ui.selectable_value(&mut self.fat_part, Some((*n, p.offset)), crate::trf!("Disque {n} · {} · {}", "Disk {n} · {} · {}", p.title(), fmt_size(p.size)));
                }
            });
            ui.horizontal(|ui| {
                ui.label(crate::tr!("Copie temporaire des fichiers sur :", "Temporary copy of the files on:"));
                letter_combo(ui, "tmp1", &mut self.temp_drive, &temp_choices, crate::tr!("choisir un lecteur", "choose a drive"));
                if let Some(t) = self.temp_drive {
                    if let Some(f) = letters.iter().find(|l| l.0 == t) {
                        ui.label(RichText::new(format!("{} libres", fmt_size(f.1))).color(MUTED()));
                    }
                }
            });
            if primary(ui, crate::tr!("Ajouter la conversion à la file d'attente", "Add the conversion to the queue"), self.fat_part.is_some() && self.temp_drive.is_some()) {
                let (n, o) = self.fat_part.unwrap();
                acts.push(Act::Add(Op::NtfsToFat32 { disk: n, offset: o, temp: self.temp_drive.unwrap() }));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Disque dynamique en disque de base", "Dynamic disk to basic disk")).size(15.0).strong());
            ui.label(RichText::new(crate::tr!("Les volumes simples sont copiés à part, le disque est converti, puis les volumes sont recréés avec les mêmes lettres et remplis à nouveau.", "Simple volumes are copied aside, the disk is converted, then the volumes are recreated with the same letters and filled again.")).color(MUTED()));
            disk_combo(ui, "dyn", &disks, &mut self.dyn_disk, |d| d.dynamic);
            if !disks.iter().any(|d| d.dynamic) {
                ui.label(RichText::new(crate::tr!("Aucun disque dynamique sur ce PC.", "No dynamic disk on this PC.")).color(MUTED()));
            }
            ui.horizontal(|ui| {
                ui.label(crate::tr!("Copie temporaire sur :", "Temporary copy on:"));
                letter_combo(ui, "tmp2", &mut self.temp_drive, &temp_choices, crate::tr!("choisir un lecteur", "choose a drive"));
            });
            if primary(ui, crate::tr!("Ajouter la conversion à la file d'attente", "Add the conversion to the queue"), self.dyn_disk.is_some() && self.temp_drive.is_some()) {
                acts.push(Act::Add(Op::DynamicToBasic { disk: self.dyn_disk.unwrap(), temp: self.temp_drive.unwrap() }));
            }
        });
    }

    fn page_winpe(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, crate::tr!("Disque de démarrage WinPE", "WinPE boot disk"), crate::tr!("Une clé USB ou une image ISO qui démarre n'importe quel PC — même sans Windows, ou sous un vieux Windows (XP, Vista, 7, Server 2003/2008) — avec DiskCraft prêt à l'emploi.", "A USB stick or an ISO image that starts any PC — even without Windows, or with an old Windows (XP, Vista, 7, Server 2003/2008) — with DiskCraft ready to use."));
        let (adk, pe) = winpe::adk_ready();
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("1. Outils WinPE gratuits de Microsoft", "1. Microsoft's free WinPE tools")).strong());
            if adk && pe {
                ui.label(RichText::new(crate::tr!("✔ Installés", "✔ Installed")).color(ACCENT));
            } else {
                ui.label(RichText::new(crate::tr!("Les outils « Windows ADK » et « WinPE » ne sont pas installés (environ 4 Go, gratuits).", "The \"Windows ADK\" and \"WinPE\" tools are not installed (about 4 GB, free).")).color(MUTED()));
                if primary(ui, crate::tr!("Installer les outils", "Install the tools"), true) {
                    acts.push(Act::RunNow(winpe::install_steps(), crate::tr!("Installation des outils WinPE", "Installing the WinPE tools").into()));
                }
            }
        });
        ui.add_space(10.0);
        let usb: Vec<char> = self.preview.iter().filter(|d| d.bus.eq_ignore_ascii_case("USB") && !d.is_system).flat_map(|d| d.parts.iter().filter_map(|p| p.letter)).collect();
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("2. Où écrire le disque de démarrage ?", "2. Where should the boot disk be written?")).strong());
            ui.radio_value(&mut self.pe_iso, true, crate::tr!("Fichier image ISO (à graver ou utiliser dans une machine virtuelle)", "ISO image file (to burn or use in a virtual machine)"));
            ui.radio_value(&mut self.pe_iso, false, crate::tr!("Clé USB (elle sera effacée)", "USB stick (it will be erased)"));
            if self.pe_iso {
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Fichier :", "File:"));
                    ui.add(egui::TextEdit::singleline(&mut self.pe_path).desired_width(420.0));
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Clé USB :", "USB stick:"));
                    letter_combo(ui, "usb", &mut self.pe_usb, &usb, crate::tr!("choisir", "choose"));
                });
                if usb.is_empty() {
                    ui.label(RichText::new(crate::tr!("Branchez une clé USB puis cliquez sur « Actualiser ».", "Plug in a USB stick, then click \"Refresh\".")).color(MUTED()));
                }
            }
        });
        ui.add_space(10.0);
        let ready = adk && pe && ((self.pe_iso && !self.pe_path.trim().is_empty()) || (!self.pe_iso && self.pe_usb.is_some()));
        if primary(ui, crate::tr!("Créer le disque de démarrage", "Create the boot disk"), ready) {
            let t = if self.pe_iso { winpe::Target::Iso(self.pe_path.trim().to_string()) } else { winpe::Target::Usb(self.pe_usb.unwrap()) };
            acts.push(Act::RunNow(winpe::build_steps(&t), crate::tr!("Création du disque de démarrage", "Creating the boot disk").into()));
        }
    }

    fn page_support(&mut self, ui: &mut egui::Ui) {
        heading(ui, crate::tr!("Aide & support gratuit", "Free help & support"), crate::tr!("DiskCraft est un logiciel libre : l'aide est gratuite et ouverte à tous.", "DiskCraft is free software: help is free and open to everyone."));
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Obtenir de l'aide", "Get help")).size(15.0).strong());
            ui.horizontal_wrapped(|ui| {
                if primary(ui, crate::tr!("Poser une question / signaler un problème", "Ask a question / report a problem"), true) {
                    open_url("https://github.com/Wikiabo-Labs/CraftDisk/issues");
                }
                if ui.button(crate::tr!("Guide en ligne", "Online guide")).clicked() {
                    open_url("https://github.com/Wikiabo-Labs/CraftDisk#readme");
                }
                if ui.button(crate::tr!("Copier les informations de mes disques", "Copy my disk information")).clicked() {
                    let mut s = format!("DiskCraft {}\n", env!("CARGO_PKG_VERSION"));
                    for d in &self.preview {
                        s += &crate::trf!("Disque {} : {} {} {}\n", "Disk {}: {} {} {}\n", d.number, d.model, fmt_size(d.size), d.type_name());
                        for p in &d.parts {
                            s += &format!("  {} {} {} {}\n", p.title(), p.fs.name(), fmt_size(p.size), p.kind.name());
                        }
                    }
                    ui.output_mut(|o| o.copied_text = s);
                }
            });
            ui.label(RichText::new(crate::tr!("Joignez les informations copiées et le journal de l'opération : la communauté vous répond gratuitement.", "Attach the copied information and the operation log: the community answers for free.")).color(MUTED()).size(12.0));
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Questions fréquentes", "Frequently asked questions")).size(15.0).strong());
            let faq = [
                (crate::tr!("Mes données sont-elles en sécurité ?", "Is my data safe?"), crate::tr!("Rien n'est modifié avant que vous cliquiez sur « Exécuter ». Les opérations qui gardent les données (redimensionner, fusionner, convertir, cloner) sont conçues pour ne rien perdre, mais une coupure de courant pendant l'opération peut tout casser : faites toujours une sauvegarde avant.", "Nothing is changed before you click \"Execute\". Operations that keep data (resize, merge, convert, clone) are designed to lose nothing, but a power cut during an operation can break everything: always make a backup first.")),
                (crate::tr!("Pourquoi Windows demande-t-il une autorisation ?", "Why does Windows ask for permission?"), crate::tr!("Modifier un disque demande les droits d'administrateur : Windows les demande une seule fois, à l'ouverture de DiskCraft. Chaque file d'attente est ensuite exécutée par un petit assistant qui écrit un journal.", "Changing a disk needs administrator rights: Windows asks for them once, when DiskCraft opens. Each queue is then run by a small helper that writes a log.")),
                (crate::tr!("Je ne peux pas agrandir C: : pourquoi ?", "Why can't I extend C:?"), crate::tr!("Il faut un espace non alloué juste après C:. Souvent une partition de récupération est placée entre les deux : supprimez ou déplacez la partition voisine, puis agrandissez C:.", "There must be unallocated space right after C:. Often a recovery partition sits in between: delete or move that neighbouring partition, then extend C:.")),
                (crate::tr!("Mon nouveau SSD est plus petit que mon disque dur.", "My new SSD is smaller than my hard drive."), crate::tr!("Réduisez d'abord la dernière partition du disque actuel (Redimensionner) pour que tout tienne, puis lancez la migration.", "First shrink the last partition of the current disk (Resize) so everything fits, then start the migration.")),
                (crate::tr!("Après la migration, le PC démarre toujours sur l'ancien disque.", "After the migration, the PC still starts from the old disk."), crate::tr!("Ouvrez le menu de démarrage du PC (souvent F12, F8 ou Échap au démarrage) et choisissez le nouveau disque, ou débranchez l'ancien.", "Open the PC's boot menu (often F12, F8 or Esc at startup) and choose the new disk, or unplug the old one.")),
                (crate::tr!("Quelles versions de Windows sont prises en charge ?", "Which versions of Windows are supported?"), crate::tr!("DiskCraft fonctionne sous Windows 10 et 11 et Windows Server 2016, 2019, 2022 et 2025. Pour un PC sous un Windows plus ancien, créez le disque de démarrage WinPE : il démarre n'importe quel PC.", "DiskCraft runs on Windows 10 and 11 and Windows Server 2016, 2019, 2022 and 2025. For a PC with an older Windows, create the WinPE boot disk: it starts any PC.")),
            ];
            for (q, a) in faq {
                egui::CollapsingHeader::new(RichText::new(q).strong()).show(ui, |ui| {
                    ui.label(a);
                });
            }
        });
    }

    fn page_health(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            crate::tr!("Santé des disques", "Disk health"),
            crate::tr!("Valeurs SMART relevées par Windows au démarrage de DiskCraft.", "SMART values read from Windows when DiskCraft starts."),
        );
        if self.health_loading.is_some() {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new(crate::tr!("Lecture des valeurs SMART...", "Reading SMART values...")).color(MUTED()));
            });
            ui.add_space(8.0);
        } else if !self.demo && !crate::rawio::is_elevated() {
            card(ui, |ui| {
                ui.label(RichText::new(crate::tr!("⚠  Lancez DiskCraft en administrateur pour lire la température, l'usure et les erreurs des disques.", "⚠  Run DiskCraft as administrator to read the temperature, wear and errors of the disks.")).color(PENDING));
            });
            ui.add_space(8.0);
        }
        for d in self.base.iter() {
            let h = self.health.get(&d.number);
            ui.add_space(4.0);
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::trf!("Disque {}", "Disk {}", d.number)).size(15.0).strong());
                    ui.label(RichText::new(&d.model).color(MUTED()));
                    let (text, color) = match h.map(|h| h.level()).unwrap_or(HealthLevel::Unknown) {
                        HealthLevel::Good => (crate::tr!("Bon état", "Good"), ICON_GREEN),
                        HealthLevel::Warning => (crate::tr!("À surveiller", "Needs attention"), PENDING),
                        HealthLevel::Bad => (crate::tr!("Défaillant : sauvegardez vos données", "Failing: back up your data"), DANGER),
                        HealthLevel::Unknown => (crate::tr!("Inconnu", "Unknown"), MUTED()),
                    };
                    badge(ui, text, color);
                });
                let Some(h) = h else {
                    ui.label(RichText::new(crate::tr!("Windows ne fournit aucune information de santé pour ce disque.", "Windows gives no health information for this disk.")).color(MUTED()));
                    return;
                };
                ui.add_space(4.0);
                // Only what this drive reports: a missing value is left out rather than shown as "—".
                let mut rows: Vec<(&str, String, Option<Color32>)> = Vec::new();
                let mut row = |k: &'static str, v: Option<String>, c: Option<Color32>| {
                    if let Some(v) = v {
                        rows.push((k, v, c));
                    }
                };
                let alert = |bad: bool, c: Color32| if bad { Some(c) } else { None };
                row(crate::tr!("État Windows", "Windows status"), (!h.status.is_empty()).then(|| format!("{} · {}", h.status, h.operational)), None);
                row(crate::tr!("Type", "Type"), Some(format!("{} · {}", if h.media.is_empty() { "?" } else { &h.media }, if h.bus.is_empty() { &d.bus } else { &h.bus })), None);
                row(
                    crate::tr!("Température", "Temperature"),
                    h.temperature.map(|t| match h.temperature_max {
                        Some(m) => crate::trf!("{} °C (max. {} °C)", "{} °C (max {} °C)", t, m),
                        None => format!("{t} °C"),
                    }),
                    alert(h.temperature.is_some_and(|t| t >= 70), PENDING),
                );
                row(
                    crate::tr!("Usure (SSD)", "Wear (SSD)"),
                    h.wear.map(|w| crate::trf!("{} % utilisés · {} % de durée de vie restante", "{} % used · {} % life left", w, 100u32.saturating_sub(w))),
                    alert(h.wear.is_some_and(|w| w >= 90), PENDING),
                );
                row(
                    crate::tr!("Réserve disponible", "Available spare"),
                    h.available_spare.map(|a| match h.available_spare_threshold {
                        Some(t) => crate::trf!("{} % (seuil {} %)", "{} % (threshold {} %)", a, t),
                        None => format!("{a} %"),
                    }),
                    alert(matches!((h.available_spare, h.available_spare_threshold), (Some(a), Some(t)) if t > 0 && a < t), DANGER),
                );
                row(crate::tr!("Heures de fonctionnement", "Power-on hours"), h.power_on_hours.map(|v| crate::trf!("{} h ({} jours)", "{} h ({} days)", v, v / 24)), None);
                row(crate::tr!("Mises sous tension", "Power cycles"), h.power_cycles.map(|v| v.to_string()), None);
                row(crate::tr!("Arrêts brutaux", "Unsafe shutdowns"), h.unsafe_shutdowns.map(|v| v.to_string()), None);
                row(crate::tr!("Données lues", "Data read"), h.data_read.map(fmt_size), None);
                row(crate::tr!("Données écrites", "Data written"), h.data_written.map(fmt_size), None);
                row(crate::tr!("Erreurs média / intégrité", "Media / integrity errors"), h.media_errors.map(|v| v.to_string()), alert(h.media_errors.is_some_and(|v| v > 0), PENDING));
                row(crate::tr!("Entrées du journal d'erreurs", "Error log entries"), h.error_log_entries.map(|v| v.to_string()), None);
                row(crate::tr!("Secteurs réalloués", "Reallocated sectors"), h.reallocated.map(|v| v.to_string()), alert(h.reallocated.is_some_and(|v| v > 0), PENDING));
                row(crate::tr!("Secteurs en attente", "Pending sectors"), h.pending.map(|v| v.to_string()), alert(h.pending.is_some_and(|v| v > 0), DANGER));
                row(crate::tr!("Secteurs irrécupérables", "Uncorrectable sectors"), h.uncorrectable.map(|v| v.to_string()), alert(h.uncorrectable.is_some_and(|v| v > 0), DANGER));
                row(crate::tr!("Erreurs de câble (CRC)", "Cable errors (CRC)"), h.crc_errors.map(|v| v.to_string()), alert(h.crc_errors.is_some_and(|v| v > 0), PENDING));
                let errors = |t: Option<u64>, u: Option<u64>| t.map(|t| crate::trf!("{} (non corrigées : {})", "{} (uncorrected: {})", t, u.unwrap_or(0)));
                row(crate::tr!("Erreurs de lecture", "Read errors"), errors(h.read_errors, h.read_errors_uncorrected), alert(h.read_errors_uncorrected.is_some_and(|n| n > 0), DANGER));
                row(crate::tr!("Erreurs d'écriture", "Write errors"), errors(h.write_errors, h.write_errors_uncorrected), alert(h.write_errors_uncorrected.is_some_and(|n| n > 0), DANGER));
                row(crate::tr!("Démarrages / arrêts", "Start / stop cycles"), h.start_stop_cycles.map(|v| v.to_string()), None);
                row(crate::tr!("Cycles de tête (HDD)", "Head load cycles (HDD)"), h.load_unload_cycles.map(|v| v.to_string()), None);
                row(
                    crate::tr!("Prédiction SMART", "SMART prediction"),
                    h.predict_failure.map(|p| if p { crate::tr!("Panne prévue par le disque", "Failure predicted by the drive").to_string() } else { crate::tr!("Aucune panne prévue", "No failure predicted").to_string() }),
                    alert(h.predict_failure == Some(true), DANGER),
                );
                row(crate::tr!("Firmware", "Firmware"), (!h.firmware.is_empty()).then(|| h.firmware.clone()), None);
                row(crate::tr!("Numéro de série", "Serial number"), (!h.serial.is_empty()).then(|| h.serial.trim_end_matches('.').to_string()), None);
                egui::Grid::new(("health", d.number)).num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
                    for (k, v, c) in rows {
                        ui.label(RichText::new(k).color(MUTED()));
                        ui.label(RichText::new(v).color(c.unwrap_or(TEXT())));
                        ui.end_row();
                    }
                });
                if !h.has_counters() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(crate::tr!("Ce disque ne transmet pas ses compteurs SMART (fréquent pour les disques USB).", "This disk does not report its SMART counters (common for USB disks).")).color(MUTED()).size(12.0));
                }
            });
        }
    }

    fn page_settings(&mut self, ui: &mut egui::Ui) {
        heading(ui, crate::tr!("Paramètres", "Settings"), crate::tr!("Préférences de DiskCraft.", "DiskCraft preferences."));
        card(ui, |ui| {
            ui.label(RichText::new(format!("🌐  {}", crate::tr!("Langue", "Language"))).size(15.0).strong());
            ui.label(RichText::new(crate::tr!("Le choix est mémorisé pour les prochains lancements.", "Your choice is remembered for next launches.")).color(MUTED()));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let en = crate::i18n::en();
                for (label, english) in [("English", true), ("Français", false)] {
                    let sel = en == english;
                    let b = egui::Button::new(RichText::new(label).size(13.5).color(if sel { Color32::WHITE } else { TEXT() }))
                        .fill(if sel { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0_f32, if sel { ACCENT } else { LINE() }))
                        .rounding(14.0);
                    if ui.add(b).clicked() && !sel {
                        crate::i18n::set_english(english);
                        ui.ctx().request_repaint();
                    }
                }
            });
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new(format!("🎨  {}", crate::tr!("Thème", "Theme"))).size(15.0).strong());
            ui.label(RichText::new(crate::tr!("Clair, sombre, ou comme Windows. Le choix est mémorisé.", "Light, dark, or the same as Windows. Your choice is remembered.")).color(MUTED()));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let cur = theme_pref::load();
                for (label, pref) in [
                    (crate::tr!("Suivre le système", "Follow system"), theme_pref::Pref::System),
                    (crate::tr!("Clair", "Light"), theme_pref::Pref::Light),
                    (crate::tr!("Sombre", "Dark"), theme_pref::Pref::Dark),
                ] {
                    let sel = cur == pref;
                    let b = egui::Button::new(RichText::new(label).size(13.5).color(if sel { Color32::WHITE } else { TEXT() }))
                        .fill(if sel { ACCENT } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0_f32, if sel { ACCENT } else { LINE() }))
                        .rounding(14.0);
                    if ui.add(b).clicked() && !sel {
                        theme_pref::save(pref);
                        ui.ctx().set_theme(pref.egui());
                        ui.ctx().request_repaint();
                    }
                }
            });
        });
    }

    fn page_about(&mut self, ui: &mut egui::Ui) {
        heading(ui, "DiskCraft", &crate::trf!("Version {} — gestionnaire de partitions libre et gratuit", "Version {} — a free, open-source partition manager", env!("CARGO_PKG_VERSION")));
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Fonctions", "Features")).size(15.0).strong());
            for f in [
                crate::tr!("Redimensionner, fusionner, créer, formater, supprimer, effacer et vérifier les partitions", "Resize, move, merge, create, format, delete, wipe and check partitions"),
                crate::tr!("Cloner des disques et des partitions (copie à chaud grâce aux instantanés)", "Clone disks and partitions (live copy thanks to snapshots)"),
                crate::tr!("Migrer Windows vers un SSD ou un disque dur, avec réparation du démarrage", "Migrate Windows to an SSD or a hard drive, with boot repair"),
                crate::tr!("Convertir MBR en GPT et GPT en MBR sans perte de données", "Convert MBR to GPT and GPT to MBR without data loss"),
                crate::tr!("Convertir NTFS en FAT32, formater en FAT32 au-delà de 32 Go", "Convert NTFS to FAT32, format FAT32 beyond 32 GB"),
                crate::tr!("Transformer un disque dynamique en disque de base", "Turn a dynamic disk into a basic disk"),
                crate::tr!("Créer un disque de démarrage WinPE", "Create a WinPE boot disk"),
                crate::tr!("Support technique gratuit par la communauté", "Free technical support from the community"),
            ] {
                ui.label(format!("✔  {f}"));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Compatibilité", "Compatibility")).size(15.0).strong());
            ui.label("Windows 11, Windows 10 · Windows Server 2025, 2022, 2019, 2016");
            ui.label(RichText::new(crate::tr!("Windows plus anciens (8.1, 8, 7, Vista, XP, Server 2012/2008/2003, Home Server) : utilisez le disque de démarrage WinPE, qui démarre ces PC et gère leurs disques.", "Older Windows (8.1, 8, 7, Vista, XP, Server 2012/2008/2003, Home Server): use the WinPE boot disk, which starts these PCs and manages their disks.")).color(MUTED()));
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new(crate::tr!("Licence", "License")).size(15.0).strong());
            ui.label(crate::tr!("Logiciel libre sous licence GNU GPL version 3 ou ultérieure : vous pouvez l'utiliser, l'étudier, le modifier et le partager librement, pour toujours, gratuitement.", "Free software under the GNU GPL version 3 or later: you can use, study, change and share it freely, forever, at no cost."));
        });
    }

    // ---------- dialogs ----------

    fn dialogs(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let Some(mut dlg) = self.dialog.take() else { return };
        let mut open = true;
        let mut close = false;
        let title = match &dlg {
            Dlg::Resize { .. } => crate::tr!("Redimensionner / déplacer la partition", "Resize / move partition").to_string(),
            Dlg::Create { .. } => crate::tr!("Créer une partition", "Create a partition").to_string(),
            Dlg::Format { .. } => crate::tr!("Formater la partition", "Format partition").to_string(),
            Dlg::Delete { .. } => crate::tr!("Supprimer la partition", "Delete partition").to_string(),
            Dlg::Wipe { offset: None, .. } => crate::tr!("Effacer le disque", "Wipe disk").to_string(),
            Dlg::Wipe { .. } => crate::tr!("Effacer la partition", "Wipe partition").to_string(),
            Dlg::Merge { .. } => crate::tr!("Fusionner des partitions", "Merge partitions").to_string(),
            Dlg::Letter { .. } => crate::tr!("Lettre de lecteur", "Drive letter").to_string(),
            Dlg::Label { .. } => crate::tr!("Nom de la partition", "Partition label").to_string(),
            Dlg::Check { .. } => crate::tr!("Vérifier la partition", "Check partition").to_string(),
            Dlg::Confirm { .. } => crate::tr!("Exécuter les opérations", "Execute operations").to_string(),
            Dlg::Message { title, .. } => title.clone(),
        };
        let free_letters = self.free_letters();
        let preview = self.preview.clone();
        let ops_list: Vec<String> = self.ops.iter().map(|o| o.describe(&self.base)).collect();
        let plan = if matches!(dlg, Dlg::Confirm { .. }) { Some(self.plan()) } else { None };
        egui::Window::new(RichText::new(title).strong()).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open).min_width(420.0).show(ctx, |ui| match &mut dlg {
            Dlg::Resize { disk, offset, pos, size_mb, regions, min, cur, can_resize, can_move, grab } => {
                let Some(dk) = preview.iter().find(|d| d.number == *disk) else { return };
                let cur_mb = (*cur / MIB) as f64;
                let size_of = |mb: f64| if (mb as u64) == *cur / MIB { *cur } else { (mb as u64) * MIB };
                // Region (free area, including the partition's own place) the partition sits in now.
                let ri = regions.iter().position(|r| *pos >= r.0 && *pos < r.1).unwrap_or(0);
                let (rs, re) = regions[ri];
                let total_mb = ((re - rs) / MIB) as f64;
                let (min_mb, max_mb) = if *can_resize { (((*min / MIB) as f64).min(cur_mb), total_mb) } else { (cur_mb, cur_mb) };

                ui.label(RichText::new(if *can_move {
                    crate::tr!("Faites glisser la partition (en bleu) sur la carte du disque pour la poser où vous voulez dans un espace libre, puis réglez sa taille en dessous.", "Drag the partition (in blue) on the disk map to drop it anywhere in free space, then set its size below.")
                } else {
                    crate::tr!("Faites glisser les bords de la barre du bas pour changer la taille de la partition.", "Drag the edges of the bottom bar to change the partition's size.")
                }).color(MUTED()));
                ui.add_space(6.0);

                // ---- Whole-disk map: the other partitions stay in place, the selected one is dragged.
                let width = 640.0f32;
                let h = 56.0f32;
                let (map, resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::drag());
                // Pieces of the disk without the selected partition: (start, end, partition).
                let mut pieces: Vec<(u64, u64, Option<&Partition>)> = Vec::new();
                let mut others: Vec<&Partition> = dk.parts.iter().filter(|p| p.offset != *offset && p.kind != Kind::Extended).collect();
                others.sort_by_key(|p| p.offset);
                let mut at = dk.usable_start();
                for p in &others {
                    if p.offset > at {
                        pieces.push((at, p.offset, None));
                    }
                    pieces.push((p.offset, p.end(), Some(*p)));
                    at = at.max(p.end());
                }
                if dk.usable_end() > at {
                    pieces.push((at, dk.usable_end(), None));
                }
                let disk_len = (dk.usable_end() - dk.usable_start()).max(1) as f32;
                let gap = 2.0f32;
                let avail = width - gap * pieces.len().saturating_sub(1) as f32;
                let raw: Vec<f32> = pieces
                    .iter()
                    .map(|(s, e, p)| {
                        let w = (e - s) as f32 / disk_len * avail;
                        if p.is_some() { w.max(40.0) } else if e - s >= MIB { w.max(26.0) } else { 0.0 }
                    })
                    .collect();
                let sum: f32 = raw.iter().sum::<f32>().max(1.0);
                let widths: Vec<f32> = raw.iter().map(|w| w * avail / sum).collect();
                let mut xs = Vec::new();
                let mut x = map.left();
                for w in &widths {
                    xs.push(x);
                    x += w + if *w > 0.0 { gap } else { 0.0 };
                }
                let to_x = |b: u64| -> f32 {
                    for (k, (s, e, _)) in pieces.iter().enumerate() {
                        if b >= *s && b <= *e && widths[k] > 0.0 {
                            return xs[k] + (b - s) as f32 / (e - s).max(1) as f32 * widths[k];
                        }
                    }
                    if b <= dk.usable_start() { map.left() } else { map.right() }
                };
                let to_b = |px: f32| -> u64 {
                    for (k, (s, e, _)) in pieces.iter().enumerate() {
                        if widths[k] > 0.0 && px >= xs[k] && px <= xs[k] + widths[k] + gap {
                            let f = ((px - xs[k]) / widths[k]).clamp(0.0, 1.0);
                            return s + ((e - s) as f64 * f as f64) as u64;
                        }
                    }
                    if px <= map.left() { dk.usable_start() } else { dk.usable_end() }
                };
                let painter = ui.painter().with_clip_rect(map.expand(2.0));
                for (k, (_, _, p)) in pieces.iter().enumerate() {
                    if widths[k] <= 0.0 {
                        continue;
                    }
                    let r = egui::Rect::from_min_size(egui::pos2(xs[k], map.top()), Vec2::new(widths[k], h));
                    match p {
                        Some(p) => {
                            let c = fs_color(p);
                            painter.rect_filled(r, 5.0, c.gamma_multiply(0.22));
                            painter.rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(r.width(), 4.0)), Rounding { nw: 5.0, ne: 5.0, sw: 0.0, se: 0.0 }, c);
                            painter.rect_stroke(r, 5.0, Stroke::new(1.0_f32, c.gamma_multiply(0.5)));
                            let pc = painter.with_clip_rect(r.shrink(2.0));
                            pc.text(r.left_top() + Vec2::new(6.0, 12.0), Align2::LEFT_TOP, p.title(), FontId::proportional(12.0), TEXT());
                            pc.text(r.left_top() + Vec2::new(6.0, 30.0), Align2::LEFT_TOP, fmt_size(p.size), FontId::proportional(11.0), MUTED());
                        }
                        None => {
                            painter.rect_filled(r, 5.0, TRACK());
                            let pc = painter.with_clip_rect(r);
                            let mut xx = r.left() - h;
                            while xx < r.right() {
                                pc.line_segment([egui::pos2(xx, r.bottom()), egui::pos2(xx + h, r.top())], Stroke::new(1.0_f32, HATCH()));
                                xx += 8.0;
                            }
                        }
                    }
                }
                // Where it may go, while dragging.
                if grab.is_some() {
                    for (s, e) in regions.iter() {
                        let r = egui::Rect::from_min_max(egui::pos2(to_x(*s), map.top()), egui::pos2(to_x(*e), map.bottom()));
                        painter.rect_stroke(r, 5.0, Stroke::new(1.5_f32, ICON_GREEN));
                    }
                }
                let size_b = size_of(*size_mb);
                let sx0 = to_x(*pos);
                let sx1 = to_x(*pos + size_b).max(sx0 + 30.0);
                let sel = egui::Rect::from_min_max(egui::pos2(sx0, map.top() - 2.0), egui::pos2(sx1, map.bottom() + 2.0));
                painter.rect_filled(sel, 5.0, ACCENT.gamma_multiply(0.85));
                painter.rect_stroke(sel, 5.0, Stroke::new(2.0_f32, ACCENT_DARK()));
                let pc = painter.with_clip_rect(sel.shrink(2.0));
                let title = dk.part_at(*offset).map(|i| dk.parts[i].title()).unwrap_or_default();
                pc.text(sel.left_top() + Vec2::new(6.0, 13.0), Align2::LEFT_TOP, title, FontId::proportional(12.0), Color32::WHITE);
                pc.text(sel.left_top() + Vec2::new(6.0, 31.0), Align2::LEFT_TOP, fmt_size(size_b), FontId::proportional(11.0), Color32::WHITE);
                if *can_move {
                    if let Some(pp) = resp.interact_pointer_pos() {
                        if resp.drag_started() && sel.expand(4.0).contains(pp) {
                            *grab = Some(to_b(pp.x).saturating_sub(*pos));
                        }
                        if let (true, Some(g)) = (resp.dragged(), *grab) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            let target = to_b(pp.x).saturating_sub(g);
                            // Snap into the free area closest to where the pointer is.
                            let mut best: Option<(u64, u64)> = None;
                            for (s, e) in regions.iter() {
                                if e - s < size_b {
                                    continue;
                                }
                                let c = align_down(target.clamp(*s, e - size_b), MIB).max(*s);
                                let dist = c.abs_diff(target);
                                if best.map_or(true, |(_, bd)| dist < bd) {
                                    best = Some((c, dist));
                                }
                            }
                            if let Some((c, _)) = best {
                                *pos = c;
                            }
                        }
                    }
                    if resp.drag_stopped() {
                        *grab = None;
                    }
                    if resp.hovered() && grab.is_none() {
                        if let Some(hp) = resp.hover_pos() {
                            if sel.contains(hp) {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            }
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::trf!("Disque {} · {} · {}", "Disk {} · {} · {}", dk.number, fmt_size(dk.size), dk.type_name())).size(12.0).color(MUTED()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(*can_resize, egui::Button::new(crate::tr!("Remplir tout l'espace non alloué", "Fill all unallocated space")).rounding(6.0)).clicked() {
                            if *can_move {
                                *pos = rs;
                            }
                            *size_mb = ((re - *pos) / MIB) as f64;
                        }
                    });
                });

                // ---- Zoom on the free area it sits in: drag the edges to resize, the body to slide.
                ui.add_space(8.0);
                let (rs, re) = regions[regions.iter().position(|r| *pos >= r.0 && *pos < r.1).unwrap_or(0)];
                let total_mb = ((re - rs) / MIB) as f64;
                let mut start_mb = ((*pos - rs) / MIB) as f64;
                ui.label(RichText::new(crate::trf!("Espace où se trouve la partition : {}", "Space the partition sits in: {}", fmt_size(re - rs))).strong());
                let zw = 640.0f32;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(zw, 46.0), Sense::hover());
                let px_per_mb = zw / total_mb.max(1.0) as f32;
                let x0 = rect.left() + start_mb as f32 * px_per_mb;
                let x1 = x0 + (*size_mb as f32 * px_per_mb).max(16.0);
                let part = egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1.min(rect.right()), rect.bottom()));
                let p = ui.painter();
                p.rect_filled(rect, 6.0, TRACK());
                let mut xx = rect.left() - 46.0;
                while xx < rect.right() {
                    p.with_clip_rect(rect).line_segment([egui::pos2(xx, rect.bottom()), egui::pos2(xx + 46.0, rect.top())], Stroke::new(1.0_f32, HATCH()));
                    xx += 8.0;
                }
                p.rect_filled(part, 6.0, Color32::from_rgb(0x2f, 0x7f, 0xd8).gamma_multiply(0.85));
                p.rect_stroke(part, 6.0, Stroke::new(2.0_f32, ACCENT_DARK()));
                for hx in [part.left() + 1.0, part.right() - 1.0] {
                    p.circle_filled(egui::pos2(hx, part.center().y), 7.0, ACCENT_DARK());
                    p.text(egui::pos2(hx, part.center().y), Align2::CENTER_CENTER, "‖", FontId::proportional(10.0), NAV());
                }
                p.text(part.center(), Align2::CENTER_CENTER, fmt_size((*size_mb as u64) * MIB), FontId::proportional(13.0), Color32::WHITE);
                let edge = 9.0;
                let lh = egui::Rect::from_min_max(part.left_top() - Vec2::new(4.0, 0.0), egui::pos2(part.left() + edge, part.bottom()));
                let rh = egui::Rect::from_min_max(egui::pos2(part.right() - edge, part.top()), part.right_bottom() + Vec2::new(4.0, 0.0));
                let body = egui::Rect::from_min_max(egui::pos2(part.left() + edge, part.top()), egui::pos2(part.right() - edge, part.bottom()));
                let id = ui.id().with("resize_bar");
                let rl = ui.interact(lh, id.with("l"), Sense::drag());
                let rr = ui.interact(rh, id.with("r"), Sense::drag());
                let rb = ui.interact(body, id.with("b"), Sense::drag());
                let dmb = |r: &egui::Response| (r.drag_delta().x / px_per_mb) as f64;
                if *can_resize {
                    if rl.hovered() || rr.hovered() || rl.dragged() || rr.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if rl.dragged() && *can_move {
                        let end = start_mb + *size_mb;
                        let ns = (start_mb + dmb(&rl)).clamp(0.0, (end - min_mb).max(0.0)).max(end - max_mb);
                        *size_mb = end - ns;
                        start_mb = ns;
                    }
                    if rr.dragged() {
                        *size_mb = (*size_mb + dmb(&rr)).clamp(min_mb, (total_mb - start_mb).min(max_mb).max(min_mb));
                    }
                }
                if *can_move {
                    if rb.hovered() || rb.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                    if rb.dragged() {
                        start_mb = (start_mb + dmb(&rb)).clamp(0.0, (total_mb - *size_mb).max(0.0));
                    }
                }
                ui.add_space(6.0);
                egui::Grid::new("resize_grid").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(crate::tr!("Espace non alloué avant :", "Unallocated space before:"));
                    ui.add_enabled(*can_move, egui::DragValue::new(&mut start_mb).range(0.0..=(total_mb - *size_mb).max(0.0)).speed(16.0).suffix(crate::tr!(" Mo", " MB")));
                    ui.label(RichText::new(fmt_size((start_mb as u64) * MIB)).color(MUTED()));
                    ui.end_row();
                    ui.label(crate::tr!("Taille de la partition :", "Partition size:"));
                    ui.add_enabled(*can_resize, egui::DragValue::new(size_mb).range(min_mb..=(total_mb - start_mb).min(max_mb).max(min_mb)).speed(16.0).suffix(crate::tr!(" Mo", " MB")));
                    ui.label(RichText::new(fmt_size((*size_mb as u64) * MIB)).color(MUTED()));
                    ui.end_row();
                    let mut after = (total_mb - start_mb - *size_mb).max(0.0);
                    ui.label(crate::tr!("Espace non alloué après :", "Unallocated space after:"));
                    if ui.add_enabled(*can_resize || *can_move, egui::DragValue::new(&mut after).range(0.0..=(total_mb - start_mb - min_mb).max(0.0)).speed(16.0).suffix(crate::tr!(" Mo", " MB"))).changed() {
                        if *can_resize {
                            *size_mb = (total_mb - start_mb - after).clamp(min_mb, max_mb);
                        } else {
                            start_mb = (total_mb - *size_mb - after).clamp(0.0, (total_mb - *size_mb).max(0.0));
                        }
                    }
                    ui.label(RichText::new(fmt_size((after as u64) * MIB)).color(MUTED()));
                    ui.end_row();
                });
                *size_mb = size_mb.round();
                start_mb = start_mb.round();
                if *can_move {
                    *pos = (rs + (start_mb as u64) * MIB).min(re.saturating_sub(size_of(*size_mb))).max(rs);
                }
                if !*can_resize {
                    ui.label(RichText::new(crate::tr!("Ce système de fichiers peut être déplacé mais pas redimensionné (seul NTFS se redimensionne).", "This file system can be moved but not resized (only NTFS can be resized).")).size(12.0).color(MUTED()));
                }
                if !*can_move {
                    ui.label(RichText::new(crate::tr!("Windows utilise cette partition : elle peut changer de taille mais pas de place. Pour la déplacer, démarrez sur le disque de démarrage DiskCraft.", "Windows is using this partition: its size can change but not its place. To move it, start the PC from the DiskCraft boot disk.")).size(12.0).color(MUTED()));
                }
                let moving = *pos != *offset;
                if moving {
                    ui.label(RichText::new(crate::tr!("⚠ Déplacer recopie toutes les données de la partition : comptez plusieurs minutes et ne coupez surtout pas le courant.", "⚠ Moving copies all of the partition's data: allow several minutes and do not cut the power.")).color(PENDING));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if primary(ui, "OK", true) {
                        let new_size = size_of(*size_mb);
                        acts.push(Act::Add(Op::Resize { disk: *disk, offset: *offset, new_offset: *pos, new_size }));
                        close = true;
                    }
                    if ui.add(egui::Button::new(crate::tr!("Annuler", "Cancel")).rounding(6.0)).clicked() {
                        close = true;
                    }
                });
            }
            Dlg::Create { disk, offset, max, size_mb, fs, label, letter } => {
                let maxm = (*max / MIB) as f64;
                ui.add(egui::Slider::new(size_mb, 1.0..=maxm.max(1.0)).text(crate::tr!("Mo", "MB")).step_by(1.0));
                fs_choice(ui, fs);
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Nom :", "Label:"));
                    ui.text_edit_singleline(label);
                });
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Lettre :", "Letter:"));
                    letter_combo(ui, "newl", letter, &free_letters, crate::tr!("aucune", "none"));
                });
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::Create { disk: *disk, offset: *offset, size: (*size_mb as u64 * MIB).min(*max), fs: fs.clone(), label: label.clone(), letter: *letter }));
                    close = true;
                }
            }
            Dlg::Format { disk, offset, fs, label, cluster, quick } => {
                fs_choice(ui, fs);
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Nom :", "Label:"));
                    ui.text_edit_singleline(label);
                });
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Taille de cluster :", "Cluster size:"));
                    let txt = if *cluster == 0 { crate::tr!("Par défaut", "Default").to_string() } else { crate::trf!("{} Ko", "{} KB", *cluster as f32 / 1024.0) };
                    egui::ComboBox::from_id_salt("clu").selected_text(txt).show_ui(ui, |ui| {
                        ui.selectable_value(cluster, 0, crate::tr!("Par défaut", "Default"));
                        for c in [512u32, 1024, 2048, 4096, 8192, 16384, 32768, 65536] {
                            ui.selectable_value(cluster, c, crate::trf!("{} Ko", "{} KB", c as f32 / 1024.0));
                        }
                    });
                });
                ui.checkbox(quick, crate::tr!("Formatage rapide", "Quick format"));
                ui.label(RichText::new(crate::tr!("⚠ Toutes les données de la partition seront perdues.", "⚠ All data on the partition will be lost.")).color(DANGER));
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::Format { disk: *disk, offset: *offset, fs: fs.clone(), label: label.clone(), cluster: *cluster, quick: *quick }));
                    close = true;
                }
            }
            Dlg::Delete { disk, offset } => {
                ui.label(crate::tr!("La partition deviendra de l'espace non alloué.", "The partition will become unallocated space."));
                ui.label(RichText::new(crate::tr!("⚠ Toutes ses données seront perdues.", "⚠ All its data will be lost.")).color(DANGER));
                if primary(ui, crate::tr!("Supprimer", "Delete"), true) {
                    acts.push(Act::Add(Op::Delete { disk: *disk, offset: *offset }));
                    close = true;
                }
            }
            Dlg::Wipe { disk, offset, passes } => {
                ui.label(crate::tr!("Les données sont écrasées et ne pourront plus être récupérées, même avec un logiciel spécialisé.", "The data is overwritten and can no longer be recovered, even with specialised software."));
                ui.add(egui::Slider::new(passes, 1..=10).text(crate::tr!("passe(s)", "pass(es)")));
                ui.label(RichText::new(crate::tr!("Chaque passe sauf la dernière écrit des données aléatoires ; la dernière écrit des zéros.", "Every pass but the last writes random data; the last one writes zeros.")).size(12.0).color(MUTED()));
                if primary(ui, crate::tr!("Effacer", "Wipe"), true) {
                    acts.push(Act::Add(match offset {
                        Some(o) => Op::WipePart { disk: *disk, offset: *o, passes: *passes },
                        None => Op::WipeDisk { disk: *disk, passes: *passes },
                    }));
                    close = true;
                }
            }
            Dlg::Merge { disk, first } => {
                if let Some(d) = preview.iter().find(|d| d.number == *disk) {
                    if let (Some(i), Some(j)) = (d.part_at(*first), d.next_part(*first)) {
                        ui.label(crate::trf!("{} et {} deviendront une seule partition de {}.", "{} and {} will become a single {} partition.", d.parts[i].title(), d.parts[j].title(), fmt_size(d.parts[j].end() - d.parts[i].offset)));
                        if let Some(l) = d.parts[j].letter {
                            ui.label(crate::trf!("Les fichiers de {l}: seront copiés à la racine de la première partition. Un fichier dont le nom existe déjà y sera renommé avec la date et l'heure (ex. test-11-23-25-20-11-2026.txt). {l}: n'est supprimée qu'une fois tous ses fichiers vérifiés.", "The files of {l}: will be copied to the root of the first partition. A file whose name already exists there is renamed with the date and time (e.g. test-11-23-25-20-11-2026.txt). {l}: is deleted only once all its files are checked."));
                        }
                    }
                }
                if primary(ui, crate::tr!("Fusionner", "Merge"), true) {
                    acts.push(Act::Add(Op::Merge { disk: *disk, first: *first }));
                    close = true;
                }
            }
            Dlg::Letter { disk, offset, letter } => {
                let mut choices = free_letters.clone();
                if let Some(l) = *letter {
                    if !choices.contains(&l) {
                        choices.insert(0, l);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(crate::tr!("Lettre :", "Letter:"));
                    letter_combo(ui, "letter", letter, &choices, crate::tr!("aucune (masquer)", "none (hide)"));
                });
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::SetLetter { disk: *disk, offset: *offset, letter: *letter }));
                    close = true;
                }
            }
            Dlg::Label { disk, offset, label } => {
                ui.text_edit_singleline(label);
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::SetLabel { disk: *disk, offset: *offset, label: label.clone() }));
                    close = true;
                }
            }
            Dlg::Check { disk, offset, mode } => {
                ui.radio_value(mode, 0, crate::tr!("Rechercher les erreurs (sans rien modifier)", "Look for errors (without changing anything)"));
                ui.radio_value(mode, 1, crate::tr!("Rechercher et réparer les erreurs", "Look for and repair errors"));
                ui.radio_value(mode, 2, crate::tr!("Test de surface (lire chaque secteur)", "Surface test (read every sector)"));
                ui.label(RichText::new(crate::tr!("La vérification se lance tout de suite ; elle ne passe pas par la file d'attente.", "The check starts right away; it does not go through the queue.")).size(12.0).color(MUTED()));
                if primary(ui, crate::tr!("Lancer", "Start"), true) {
                    if let Some(d) = preview.iter().find(|d| d.number == *disk) {
                        if let Some(i) = d.part_at(*offset) {
                            acts.push(Act::RunNow(ops::check_steps(d, &d.parts[i], *mode), crate::tr!("Vérification de la partition", "Checking the partition").into()));
                        }
                    }
                    close = true;
                }
            }
            Dlg::Confirm { backup_ok } => {
                ui.label(crate::tr!("Ces opérations vont être appliquées à vos disques :", "These operations will be applied to your disks:"));
                for (i, o) in ops_list.iter().enumerate() {
                    ui.label(format!("{}. {o}", i + 1));
                }
                let ok = matches!(plan, Some(Ok(_)));
                match &plan {
                    Some(Ok(steps)) => {
                        egui::CollapsingHeader::new(crate::trf!("Voir les {} étapes détaillées", "Show the {} detailed steps", steps.len())).show(ui, |ui| {
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                for (i, s) in steps.iter().enumerate() {
                                    ui.label(RichText::new(format!("{}. {}", i + 1, s.title())).strong());
                                    if let Step::Ps { script } = s {
                                        ui.label(RichText::new(script).monospace().size(11.0));
                                    }
                                }
                            });
                        });
                    }
                    Some(Err(e)) => {
                        ui.label(RichText::new(e).color(DANGER));
                    }
                    None => {}
                }
                ui.add_space(6.0);
                ui.checkbox(backup_ok, crate::tr!("J'ai sauvegardé mes fichiers importants et le PC restera branché", "I have backed up my important files and the PC will stay plugged in"));
                ui.label(RichText::new(crate::tr!("Ne coupez pas le courant pendant l'opération.", "Do not cut the power during the operation.")).size(12.0).color(MUTED()));
                if primary(ui, crate::tr!("Exécuter maintenant", "Execute now"), *backup_ok && ok) {
                    acts.push(Act::Execute);
                    close = true;
                }
            }
            Dlg::Message { text, .. } => {
                ui.label(text.as_str());
                if primary(ui, "OK", true) {
                    close = true;
                }
            }
        });
        if open && !close && self.dialog.is_none() {
            self.dialog = Some(dlg);
        }
    }

    fn run_window(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let Some(r) = &self.run else { return };
        egui::Window::new(RichText::new(&r.title).strong()).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).min_width(480.0).show(ctx, |ui| {
            match r.done {
                None => {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(&r.step);
                    });
                    ui.add(egui::ProgressBar::new(r.pct).show_percentage());
                    ui.label(RichText::new(crate::tr!("Ne coupez pas le courant et ne débranchez pas les disques.", "Do not cut the power or unplug the disks.")).color(MUTED()).size(12.0));
                }
                Some(true) => {
                    ui.label(RichText::new(crate::tr!("✔ Terminé avec succès", "✔ Completed successfully")).size(16.0).color(ACCENT).strong());
                }
                Some(false) => {
                    ui.label(RichText::new(crate::tr!("⚠ L'opération s'est arrêtée", "⚠ The operation stopped")).size(16.0).color(DANGER).strong());
                }
            }
            egui::ScrollArea::vertical().max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
                for l in &r.lines {
                    ui.label(RichText::new(l).monospace().size(11.5));
                }
            });
            if r.done.is_some() {
                ui.horizontal(|ui| {
                    if primary(ui, crate::tr!("Fermer", "Close"), true) {
                        acts.push(Act::Reload);
                    }
                    if ui.button(crate::tr!("Ouvrir le journal", "Open the log")).clicked() {
                        open_url(&r.log.display().to_string());
                    }
                });
            }
        });
    }
}

fn fs_choice(ui: &mut egui::Ui, fs: &mut Fs) {
    ui.horizontal(|ui| {
        ui.label(crate::tr!("Système de fichiers :", "File system:"));
        for f in [Fs::Ntfs, Fs::Fat32, Fs::ExFat] {
            let name = f.name();
            ui.selectable_value(fs, f, name);
        }
    });
}

fn open_url(url: &str) {
    let mut c = std::process::Command::new("cmd");
    c.args(["/C", "start", "", url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let _ = c.spawn();
}

/// Theme chosen in Settings, remembered next to the language choice.
mod theme_pref {
    #[derive(Clone, Copy, PartialEq)]
    pub enum Pref {
        System,
        Light,
        Dark,
    }
    impl Pref {
        pub fn egui(self) -> egui::ThemePreference {
            match self {
                Pref::System => egui::ThemePreference::System,
                Pref::Light => egui::ThemePreference::Light,
                Pref::Dark => egui::ThemePreference::Dark,
            }
        }
    }
    fn file() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("APPDATA")?;
        Some(std::path::PathBuf::from(base).join("DiskCraft").join("theme.txt"))
    }
    /// Saved choice; dark (DiskCraft's original look) when nothing was chosen yet.
    pub fn load() -> Pref {
        let saved = std::env::var("DISKCRAFT_THEME").ok().or_else(|| file().and_then(|p| std::fs::read_to_string(p).ok()));
        match saved.as_deref().map(str::trim) {
            Some("system") => Pref::System,
            Some("light") => Pref::Light,
            _ => Pref::Dark,
        }
    }
    pub fn save(p: Pref) {
        if let Some(f) = file() {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(f, match p {
                Pref::System => "system",
                Pref::Light => "light",
                Pref::Dark => "dark",
            });
        }
    }
}
