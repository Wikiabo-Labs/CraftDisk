//! The DiskCraft window.

use crate::model::*;
use crate::ops::{self, Op};
use crate::plan::{Plan, Step};
use crate::winpe;
use egui::{Align2, Color32, FontId, RichText, Rounding, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

// Colours.
const NAV: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x1f);
const NAV_HI: Color32 = Color32::from_rgb(0x2b, 0x2d, 0x33);
const ACCENT: Color32 = Color32::from_rgb(0x4a, 0x7d, 0xf7);
const ACCENT_DARK: Color32 = Color32::from_rgb(0x8f, 0xb0, 0xff);
const BRAND: Color32 = Color32::from_rgb(0x22, 0xa0, 0x78);
const BG: Color32 = Color32::from_rgb(0x20, 0x21, 0x25);
const CARD: Color32 = Color32::from_rgb(0x28, 0x29, 0x2e);
const CARD_HI: Color32 = Color32::from_rgb(0x31, 0x33, 0x39);
const LINE: Color32 = Color32::from_rgb(0x38, 0x3a, 0x41);
const TEXT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xee);
const MUTED: Color32 = Color32::from_rgb(0x9b, 0xa1, 0xab);
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
    Partitions,
    Clone,
    Migrate,
    Convert,
    WinPe,
    Support,
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
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.extreme_bg_color = Color32::from_rgb(0x1b, 0x1c, 0x20);
        style.visuals.faint_bg_color = Color32::from_rgb(0x2c, 0x2e, 0x34);
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
        style.visuals.widgets.inactive.weak_bg_fill = CARD_HI;
        style.visuals.widgets.inactive.bg_fill = CARD_HI;
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x3a, 0x3d, 0x45);
        style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0x44, 0x48, 0x52);
        style.visuals.window_stroke = Stroke::new(1.0_f32, LINE);
        style.visuals.hyperlink_color = ACCENT_DARK;
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = CARD;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = ACCENT.gamma_multiply(0.6);
        style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_DARK);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        style.visuals.window_rounding = Rounding::same(10.0);
        style.spacing.item_spacing = Vec2::new(8.0, 7.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        // Always the dark DiskCraft look, whatever Windows' light/dark setting is
        // (on a PC in light mode the window used to switch to the light style: grey text on dark panels).
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style.clone());
        cc.egui_ctx.set_style_of(egui::Theme::Light, style);
        if std::env::var("DISKCRAFT_TEST_LIGHT").is_ok() {
            // Check only: behave as on a PC set to light mode; the look must not change.
            cc.egui_ctx.set_theme(egui::Theme::Light);
        }
        let logo_tex = cc.egui_ctx.load_texture("logo", crate::logo::image(128), egui::TextureOptions::LINEAR);

        let docs = std::env::var("USERPROFILE").map(|p| format!(r"{p}\Documents\DiskCraft-WinPE.iso")).unwrap_or_else(|_| r"C:\DiskCraft-WinPE.iso".into());
        let mut app = App {
            base: Vec::new(),
            preview: Vec::new(),
            ops: Vec::new(),
            demo,
            loading: None,
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
            "partitions" => Page::Partitions,
            "clone" => Page::Clone,
            "migrate" => Page::Migrate,
            "convert" => Page::Convert,
            "winpe" => Page::WinPe,
            "support" => Page::Support,
            "about" => Page::About,
            _ => Page::Home,
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        if self.demo {
            self.set_disks(crate::probe::demo_disks());
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
            self.load_note = Some("Mode démonstration : ce sont des disques d'exemple, rien n'est modifié sur votre PC.".into());
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::probe::load_disks());
        });
        self.loading = Some(rx);
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
            Err(e) => self.dialog = Some(Dlg::Message { title: "Opération impossible".into(), text: e }),
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
                title: "Mode démonstration".into(),
                text: "Les disques affichés sont des exemples : rien n'a été exécuté. Relancez DiskCraft sans --demo pour travailler sur vos vrais disques.".into(),
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
            self.dialog = Some(Dlg::Message { title: "Erreur".into(), text: e.to_string() });
            return;
        }
        let _ = std::fs::write(&log_path, "");
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
        // The plan's fingerprint travels on the elevated command line, which cannot be changed after
        // the administrator agreed: the worker refuses a plan file that was altered meanwhile.
        let args = format!("--worker \"{}\" \"{}\" {}", plan_path.display(), log_path.display(), crate::worker::sha256_hex(json.as_bytes()));
        let cmdline = format!("Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden", ops::ps_quote(&exe), ops::ps_quote(&args));
        let lp = log_path.clone();
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
                let _ = std::fs::write(&lp, "ERREUR L'autorisation administrateur a été refusée : rien n'a été modifié.\nFIN ECHEC\n");
            }
        });
        self.dialog = None;
        self.run = Some(Run { log: log_path, title, lines: Vec::new(), step: "Démarrage (autorisez DiskCraft à modifier l'appareil)...".into(), pct: 0.0, done: None, queue, last_read: 0.0 });
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
                r.lines.push(s.to_string());
            } else if let Some(s) = l.strip_prefix("ERREUR ") {
                r.lines.push(format!("⚠ {s}"));
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
        .fill(CARD)
        .rounding(10.0)
        .stroke(Stroke::new(1.0, LINE))
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
        ui.label(RichText::new(sub).color(MUTED));
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
        .map(|d| format!("Disque {} · {} · {}", d.number, d.model, fmt_size(d.size)))
        .unwrap_or_else(|| "Choisir un disque...".into());
    egui::ComboBox::from_id_salt(id).width(420.0).selected_text(text).show_ui(ui, |ui| {
        for d in disks.iter().filter(|d| filter(d)) {
            ui.selectable_value(value, Some(d.number), format!("Disque {} · {} · {} · {}", d.number, d.model, fmt_size(d.size), d.type_name()));
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
                        self.load_note = Some(format!("Les disques de ce PC n'ont pas pu être lus ({}) : disques d'exemple affichés.", e.trim()));
                    }
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
        egui::CentralPanel::default().frame(egui::Frame::none().fill(BG).inner_margin(18.0)).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.page {
                Page::Home => self.page_home(ui, &mut acts),
                Page::Partitions => self.page_partitions(ui, &mut acts),
                Page::Clone => self.page_clone(ui, &mut acts),
                Page::Migrate => self.page_migrate(ui, &mut acts),
                Page::Convert => self.page_convert(ui, &mut acts),
                Page::WinPe => self.page_winpe(ui, &mut acts),
                Page::Support => self.page_support(ui),
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
                        self.start_worker(steps, format!("Exécution de {n} opération(s)"), true)
                    }
                    Err(e) => self.dialog = Some(Dlg::Message { title: "Erreur".into(), text: e }),
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
        } else if self.loading.is_some() || self.run.as_ref().is_some_and(|r| r.done.is_none()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl App {
    fn nav(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::SidePanel::left("nav").exact_width(236.0).resizable(false).frame(egui::Frame::none().fill(NAV).stroke(Stroke::new(1.0_f32, LINE)).inner_margin(egui::Margin::symmetric(12.0, 14.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::new(38.0, 38.0)).rounding(8.0));
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new("DiskCraft").size(19.0).strong().color(TEXT));
                    ui.label(RichText::new(format!("v{} · libre et gratuit", env!("CARGO_PKG_VERSION"))).size(11.0).color(MUTED));
                });
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            let items = [
                (Page::Home, "🏠", "Accueil", ICON_BLUE),
                (Page::Partitions, "🖴", "Gestionnaire de partitions", ICON_TEAL),
                (Page::Clone, "🗐", "Cloner", ICON_PURPLE),
                (Page::Migrate, "➡", "Migrer Windows", ICON_GREEN),
                (Page::Convert, "⟲", "Convertisseurs", ICON_ORANGE),
                (Page::WinPe, "💿", "Disque de démarrage", ICON_TEAL),
                (Page::Support, "❓", "Aide & support", ICON_RED),
                (Page::About, "ℹ", "À propos", MUTED),
            ];
            for (page, icon, name, color) in items {
                let sel = self.page == page;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::click());
                if sel || resp.hovered() {
                    ui.painter().rect_filled(rect, 7.0, if sel { NAV_HI } else { NAV_HI.gamma_multiply(0.6) });
                }
                if sel {
                    ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 9.0), Vec2::new(3.0, 20.0)), 2.0, ACCENT);
                }
                let col = if sel { TEXT } else { Color32::from_gray(200) };
                ui.painter().text(rect.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, icon, FontId::proportional(16.0), color);
                ui.painter().text(rect.left_center() + Vec2::new(42.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(14.0), col);
                if resp.clicked() {
                    acts.push(Act::Page(page));
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.label(RichText::new("Logiciel libre (GPL-3.0) · support gratuit").size(11.0).color(MUTED));
            });
        });
    }

    fn top(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::TopBottomPanel::top("top").exact_height(54.0).frame(egui::Frame::none().fill(CARD).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(18.0, 10.0))).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let title = match self.page {
                    Page::Home => "Accueil",
                    Page::Partitions => "Gestionnaire de partitions",
                    Page::Clone => "Cloner un disque ou une partition",
                    Page::Migrate => "Migrer Windows vers un SSD ou un disque dur",
                    Page::Convert => "Convertisseurs",
                    Page::WinPe => "Disque de démarrage WinPE",
                    Page::Support => "Aide & support gratuit",
                    Page::About => "À propos de DiskCraft",
                };
                ui.label(RichText::new(title).size(17.0).strong());
                if self.loading.is_some() {
                    ui.add(egui::Spinner::new());
                    ui.label(RichText::new("Lecture des disques...").color(MUTED));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let n = self.ops.len();
                    if primary(ui, &format!("▶  Exécuter {n} opération(s)"), n > 0) {
                        acts.push(Act::Open(Dlg::Confirm { backup_ok: false }));
                    }
                    if ui.add_enabled(n > 0, egui::Button::new("↩ Annuler").rounding(6.0)).clicked() {
                        acts.push(Act::Undo);
                    }
                    if ui.add_enabled(self.loading.is_none(), egui::Button::new("⟳ Actualiser").rounding(6.0)).clicked() {
                        acts.push(Act::Reload);
                    }
                });
            });
        });
    }

    fn queue_panel(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        egui::TopBottomPanel::bottom("queue")
            .resizable(false)
            .frame(egui::Frame::none().fill(Color32::from_rgb(0x33, 0x2b, 0x20)).stroke(Stroke::new(1.0, PENDING.gamma_multiply(0.5))).inner_margin(egui::Margin::symmetric(18.0, 10.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Opérations en attente ({})", self.ops.len())).strong().color(PENDING));
                    ui.label(RichText::new("Rien n'est modifié tant que vous n'avez pas cliqué sur « Exécuter ». Vous pouvez annuler à tout moment.").color(MUTED).size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Tout annuler").clicked() {
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
            egui::Frame::none().fill(Color32::from_rgb(0x22, 0x30, 0x4a)).rounding(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(n).color(ACCENT_DARK));
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
                        ui.painter().rect_filled(r, 7.0, if header_sel { ACCENT } else { CARD_HI });
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "🖴", FontId::proportional(18.0), if header_sel { Color32::WHITE } else { MUTED });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Disque {}", d.number)).size(15.5).strong());
                                ui.label(RichText::new(&d.model).color(MUTED));
                                if d.is_system || d.is_boot {
                                    badge(ui, "Windows", ACCENT);
                                }
                                if d.pending {
                                    badge(ui, "Modifié", PENDING);
                                }
                                if d.offline {
                                    badge(ui, "Hors ligne", DANGER);
                                }
                            });
                            let bus = if d.bus.is_empty() { "—".to_string() } else { d.bus.clone() };
                            ui.label(RichText::new(format!("{} · {} · {}", fmt_size(d.size), d.type_name(), bus)).size(12.0).color(MUTED));
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
            ui.label("Aucun disque.");
        }
    }

    fn disk_bar(&self, ui: &mut egui::Ui, d: &Disk, acts: &mut Vec<Act>) {
        let segs = d.segments();
        let width = ui.available_width();
        let h = 58.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
        if segs.is_empty() {
            ui.painter().rect_filled(rect, 6.0, CARD_HI);
            let msg = if d.size == 0 { "Aucun support inséré" } else { "Disque non initialisé : sélectionnez le disque puis « Initialiser en GPT » ou « en MBR »" };
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(13.0), MUTED);
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
                    "Non alloué".to_string(),
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
                    p.line_segment([egui::pos2(xx, r.bottom()), egui::pos2(xx + h, r.top() + 5.0)], Stroke::new(1.0_f32, Color32::from_rgb(0x44, 0x47, 0x4f)));
                    xx += 9.0;
                }
            }
            p.text(r.left_top() + Vec2::new(7.0, 13.0), Align2::LEFT_TOP, &label, FontId::proportional(13.0), TEXT);
            p.text(r.left_top() + Vec2::new(7.0, 32.0), Align2::LEFT_TOP, &sub, FontId::proportional(11.5), MUTED);
            let stroke = if sel {
                Stroke::new(2.5, ACCENT_DARK)
            } else if pending {
                Stroke::new(2.0, PENDING)
            } else if resp.hovered() {
                Stroke::new(1.5, color)
            } else {
                Stroke::new(1.0, color.gamma_multiply(0.4))
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
                    format!("{}\n{} · {}\nUtilisé : {}", pt.title(), pt.kind.name(), fmt_size(pt.size), pt.used.map(fmt_size).unwrap_or_else(|| "—".into()))
                }
                Segment::Free { size, .. } => format!("Espace non alloué : {}", fmt_size(*size)),
            });
        }
    }

    fn part_table(&self, ui: &mut egui::Ui, d: &Disk, acts: &mut Vec<Act>) {
        if d.segments().is_empty() {
            return;
        }
        egui::Grid::new(("grid", d.number)).num_columns(7).spacing([18.0, 4.0]).striped(true).min_col_width(60.0).show(ui, |ui| {
            for h in ["Partition", "Système de fichiers", "Taille", "Utilisé", "Libre", "Type", "État"] {
                ui.label(RichText::new(h).size(12.0).color(MUTED).strong());
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
                            st.push("Démarrage");
                        }
                        if p.is_system {
                            st.push("Système");
                        }
                        if p.is_active {
                            st.push("Active");
                        }
                        if p.letter.is_none() && p.kind == Kind::Data {
                            st.push("Masquée");
                        }
                        if p.pending {
                            ui.label(RichText::new("En attente").color(PENDING));
                        } else {
                            ui.label(if st.is_empty() { "Aucun".to_string() } else { st.join(", ") });
                        }
                    }
                    Segment::Free { offset, size } => {
                        if ui.selectable_label(matches!(self.sel, Sel::Free(n, o, _) if n == d.number && o == offset), RichText::new("Non alloué").italics()).clicked() {
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
        egui::SidePanel::right("side").exact_width(270.0).resizable(false).frame(egui::Frame::none().fill(CARD).stroke(Stroke::new(1.0, LINE)).inner_margin(14.0)).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.sel {
                Sel::None => {
                    ui.label(RichText::new("Sélectionnez une partition").size(15.0).strong());
                    ui.label(RichText::new("Cliquez sur une partition, un espace non alloué ou le titre d'un disque pour voir ce que vous pouvez faire.").color(MUTED));
                    ui.add_space(10.0);
                    let d = self.preview.len();
                    let p = self.all_parts().len();
                    ui.label(format!("{d} disque(s), {p} partition(s)"));
                    ui.add_space(10.0);
                    ui.label(RichText::new("Légende").strong());
                    for (name, c) in [
                        ("NTFS", Color32::from_rgb(0x2f, 0x7f, 0xd8)),
                        ("FAT32", Color32::from_rgb(0x3f, 0xa6, 0x5a)),
                        ("exFAT", Color32::from_rgb(0x1f, 0xa3, 0xa8)),
                        ("Système EFI", Color32::from_rgb(0x8a, 0x63, 0xd2)),
                        ("Récupération", Color32::from_rgb(0xd9, 0x8b, 0x2b)),
                        ("Non alloué", Color32::from_rgb(0xb4, 0xbb, 0xc6)),
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
                    ui.label(RichText::new(format!("Disque {} · {} · {}", n, p.fs.name(), fmt_size(p.size))).color(MUTED));
                    if let Some(u) = p.used {
                        ui.add(egui::ProgressBar::new(u as f32 / p.size.max(1) as f32).text(format!("{} utilisés sur {}", fmt_size(u), fmt_size(p.size))));
                    }
                    ui.add_space(8.0);
                    let prot = p.protected();
                    let ntfs = p.fs == Fs::Ntfs;
                    let can_resize = matches!(p.fs, Fs::Ntfs | Fs::Refs);
                    let can_move = !prot && !matches!(p.kind, Kind::Logical | Kind::Extended | Kind::LdmData | Kind::LdmMeta);
                    if action(ui, "↔", "Redimensionner / déplacer", can_resize || can_move) {
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
                    if action(ui, "🔗", "Fusionner avec la suivante", ntfs && d.next_part(o).is_some()) {
                        acts.push(Act::Open(Dlg::Merge { disk: n, first: o }));
                    }
                    if action(ui, "🖹", "Formater", !prot) {
                        acts.push(Act::Open(Dlg::Format { disk: n, offset: o, fs: if p.fs == Fs::Raw { Fs::Ntfs } else { p.fs.clone() }, label: p.label.clone(), cluster: 0, quick: true }));
                    }
                    if action(ui, "🗑", "Supprimer", !prot) {
                        acts.push(Act::Open(Dlg::Delete { disk: n, offset: o }));
                    }
                    if action(ui, "✖", "Effacer les données", !prot) {
                        acts.push(Act::Open(Dlg::Wipe { disk: n, offset: Some(o), passes: 1 }));
                    }
                    if action(ui, "✔", "Vérifier / test de surface", true) {
                        acts.push(Act::Open(Dlg::Check { disk: n, offset: o, mode: 0 }));
                    }
                    if action(ui, "🗐", "Cloner la partition", true) {
                        self.clone_part_mode = true;
                        self.clone_psrc = Some((n, o));
                        acts.push(Act::Page(Page::Clone));
                    }
                    if action(ui, "⟲", "Convertir NTFS en FAT32", ntfs && !prot) {
                        self.fat_part = Some((n, o));
                        acts.push(Act::Page(Page::Convert));
                    }
                    ui.separator();
                    if action(ui, "🔤", "Changer la lettre / masquer", !p.is_boot) {
                        acts.push(Act::Open(Dlg::Letter { disk: n, offset: o, letter: p.letter }));
                    }
                    if action(ui, "🏷", "Changer le nom", p.fs != Fs::Raw) {
                        acts.push(Act::Open(Dlg::Label { disk: n, offset: o, label: p.label.clone() }));
                    }
                    if action(ui, "★", "Rendre active", d.style == Style::Mbr && !p.is_active) {
                        acts.push(Act::Add(Op::SetActive { disk: n, offset: o }));
                    }
                    if prot {
                        ui.add_space(6.0);
                        ui.label(RichText::new("Windows utilise cette partition : la supprimer, la formater ou l'effacer est bloqué.").size(12.0).color(MUTED));
                    }
                }
                Sel::Free(n, o, s) => {
                    ui.label(RichText::new("Espace non alloué").size(17.0).strong());
                    ui.label(RichText::new(format!("Disque {n} · {}", fmt_size(s))).color(MUTED));
                    ui.add_space(8.0);
                    if action(ui, "＋", "Créer une partition", true) {
                        let letter = self.free_letters().first().copied();
                        acts.push(Act::Open(Dlg::Create { disk: n, offset: o, max: s, size_mb: (s / MIB) as f64, fs: Fs::Ntfs, label: String::new(), letter }));
                    }
                    if action(ui, "🗐", "Cloner une partition ici", true) {
                        self.clone_part_mode = true;
                        self.clone_pdst = Some((n, o));
                        acts.push(Act::Page(Page::Clone));
                    }
                    ui.label(RichText::new("Pour agrandir la partition placée juste avant, sélectionnez-la puis « Redimensionner ».").size(12.0).color(MUTED));
                }
                Sel::Disk(n) => {
                    let Some(d) = self.disk(n).cloned() else { return };
                    ui.label(RichText::new(format!("Disque {n}")).size(17.0).strong());
                    ui.label(RichText::new(format!("{} · {} · {}", d.model, fmt_size(d.size), d.type_name())).color(MUTED));
                    ui.add_space(8.0);
                    let sys = d.is_system || d.is_boot;
                    if action(ui, "🗐", "Cloner le disque", true) {
                        self.clone_part_mode = false;
                        self.clone_src = Some(n);
                        acts.push(Act::Page(Page::Clone));
                    }
                    if action(ui, "➡", "Migrer Windows vers un autre disque", sys) {
                        acts.push(Act::Page(Page::Migrate));
                    }
                    if action(ui, "⟲", if d.style == Style::Raw { "Initialiser en GPT" } else { "Convertir en GPT" }, d.style != Style::Gpt) {
                        acts.push(Act::Add(Op::ToGpt { disk: n }));
                    }
                    if action(ui, "⟲", if d.style == Style::Raw { "Initialiser en MBR" } else { "Convertir en MBR" }, d.style != Style::Mbr && !sys) {
                        acts.push(Act::Add(Op::ToMbr { disk: n }));
                    }
                    if action(ui, "⟲", "Convertir en disque de base", d.dynamic) {
                        self.dyn_disk = Some(n);
                        acts.push(Act::Page(Page::Convert));
                    }
                    if action(ui, "✖", "Effacer tout le disque", !sys) {
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
                ui.label(RichText::new("Bienvenue dans DiskCraft").size(27.0).strong());
                ui.label(RichText::new("Un gestionnaire de disques open source — local, gratuit et complet.").size(14.5).color(MUTED));
            });
        });
        ui.add_space(14.0);
        if let Some(n) = &self.load_note {
            egui::Frame::none().fill(Color32::from_rgb(0x22, 0x30, 0x4a)).rounding(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(n).color(ACCENT_DARK));
            });
            ui.add_space(10.0);
        }
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("👥").size(20.0).color(ICON_BLUE));
                ui.vertical(|ui| {
                    ui.label(RichText::new("Rejoindre la communauté DiskCraft").size(15.0).strong());
                    ui.label(RichText::new("Obtenez de l'aide gratuitement, proposez des idées et suivez le développement.").color(MUTED));
                });
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if primary(ui, "💬  Poser une question", true) {
                    open_url("https://github.com/diskcraft/diskcraft/discussions");
                }
                if ui.add(egui::Button::new("🐛  Signaler un problème").rounding(14.0)).clicked() {
                    open_url("https://github.com/diskcraft/diskcraft/issues");
                }
                if ui.add(egui::Button::new("</>  DiskCraft sur GitHub").rounding(14.0)).clicked() {
                    open_url("https://github.com/diskcraft/diskcraft");
                }
                if ui.add(egui::Button::new("📖  Guide d'utilisation").rounding(14.0)).clicked() {
                    acts.push(Act::Page(Page::Support));
                }
            });
        });
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.label(RichText::new("Outils recommandés").size(15.0).strong());
            ui.add_space(6.0);
            let tools = [
                ("↔", ICON_BLUE, "Redimensionner / déplacer", "Agrandir C: · Réduire · Déplacer une partition", Page::Partitions),
                ("🗐", ICON_PURPLE, "Cloner", "Disque entier · Partition · Copie à chaud", Page::Clone),
                ("➡", ICON_GREEN, "Migrer Windows", "Vers un SSD ou un nouveau disque · Démarrage réparé", Page::Migrate),
                ("⟲", ICON_ORANGE, "Convertir", "MBR / GPT · NTFS en FAT32 · Dynamique en base", Page::Convert),
                ("💿", ICON_TEAL, "Disque de démarrage", "Clé USB ou ISO WinPE avec DiskCraft", Page::WinPe),
                ("✔", ICON_RED, "Vérifier un disque", "Erreurs du système de fichiers · Test de surface", Page::Partitions),
            ];
            let w = ((ui.available_width() - 2.0 * 10.0) / 3.0).max(200.0);
            egui::Grid::new("tools").num_columns(3).spacing([10.0, 10.0]).show(ui, |ui| {
                for (k, (icon, color, title, sub, page)) in tools.iter().enumerate() {
                    let resp = egui::Frame::none()
                        .fill(CARD_HI)
                        .rounding(8.0)
                        .stroke(Stroke::new(1.0_f32, LINE))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(w - 26.0);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(*icon).size(18.0).color(*color));
                                    ui.label(RichText::new(*title).size(14.5).strong());
                                });
                                ui.add(egui::Label::new(RichText::new(*sub).size(12.0).color(MUTED)).truncate());
                                ui.label(RichText::new("Utiliser").size(12.5).strong().color(ACCENT_DARK));
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
            ui.label(RichText::new("Vos disques").size(17.0).strong());
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
                .fill(CARD)
                .rounding(8.0)
                .stroke(Stroke::new(1.0_f32, LINE))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🖴").size(20.0).color(if d.is_system || d.is_boot { ICON_GREEN } else { ICON_BLUE }));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Disque {}", d.number)).strong());
                                ui.label(RichText::new(&d.model).color(MUTED));
                                if d.is_system || d.is_boot {
                                    badge(ui, "Windows", ICON_GREEN);
                                }
                            });
                            ui.label(RichText::new(format!("{} · {} · {} partition(s)", fmt_size(d.size), d.type_name(), d.parts.len())).size(12.0).color(MUTED));
                            if d.size > 0 {
                                let frac = used as f32 / d.size as f32;
                                ui.add(egui::ProgressBar::new(frac).desired_width(380.0).text(format!("{} utilisés · {} non alloués", fmt_size(used), fmt_size(d.size.saturating_sub(allocated)))));
                            } else {
                                ui.label(RichText::new("Aucun support inséré").size(12.0).color(MUTED));
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
        heading(ui, "Cloner", "Copie exacte secteur par secteur. Les volumes en cours d'utilisation sont copiés depuis un instantané, sans redémarrer.");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.clone_part_mode, false, RichText::new("  Cloner un disque  ").size(14.0));
            ui.selectable_value(&mut self.clone_part_mode, true, RichText::new("  Cloner une partition  ").size(14.0));
        });
        ui.add_space(8.0);
        let disks = self.preview.clone();
        if !self.clone_part_mode {
            card(ui, |ui| {
                ui.label(RichText::new("1. Disque source").strong());
                disk_combo(ui, "csrc", &disks, &mut self.clone_src, |_| true);
                ui.add_space(6.0);
                ui.label(RichText::new("2. Disque cible (il sera entièrement remplacé)").strong());
                let src = self.clone_src;
                disk_combo(ui, "cdst", &disks, &mut self.clone_dst, |d| Some(d.number) != src && !d.is_system && !d.is_boot);
                ui.checkbox(&mut self.fit, "Agrandir la dernière partition pour remplir le disque cible");
                if let (Some(s), Some(t)) = (self.clone_src.and_then(|n| disks.iter().find(|d| d.number == n)), self.clone_dst.and_then(|n| disks.iter().find(|d| d.number == n))) {
                    ui.add_space(6.0);
                    ui.label(format!("À copier : {}, place disponible : {}", fmt_size(s.used_extent()), fmt_size(t.size)));
                    if !t.parts.is_empty() {
                        ui.label(RichText::new(format!("⚠ Les {} partition(s) du disque {} seront effacées.", t.parts.len(), t.number)).color(DANGER));
                    }
                }
                ui.add_space(6.0);
                if primary(ui, "Ajouter à la file d'attente", self.clone_src.is_some() && self.clone_dst.is_some()) {
                    acts.push(Act::Add(Op::CloneDisk { src: self.clone_src.unwrap(), dst: self.clone_dst.unwrap(), fit: self.fit }));
                    acts.push(Act::Page(Page::Partitions));
                }
            });
        } else {
            let parts: Vec<(u32, Partition)> = self.all_parts().into_iter().map(|(n, p)| (n, p.clone())).collect();
            card(ui, |ui| {
                ui.label(RichText::new("1. Partition source").strong());
                let txt = self
                    .clone_psrc
                    .and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o))
                    .map(|(n, p)| format!("Disque {n} · {} · {}", p.title(), fmt_size(p.size)))
                    .unwrap_or_else(|| "Choisir une partition...".into());
                egui::ComboBox::from_id_salt("psrc").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                    for (n, p) in &parts {
                        ui.selectable_value(&mut self.clone_psrc, Some((*n, p.offset)), format!("Disque {n} · {} · {} · {}", p.title(), p.fs.name(), fmt_size(p.size)));
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("2. Espace non alloué cible").strong());
                let need = self.clone_psrc.and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o)).map(|(_, p)| p.size).unwrap_or(0);
                let frees: Vec<(u32, u64, u64)> = disks
                    .iter()
                    .flat_map(|d| d.segments().into_iter().filter_map(move |s| if let Segment::Free { offset, size } = s { Some((d.number, offset, size)) } else { None }))
                    .collect();
                let txt = self
                    .clone_pdst
                    .and_then(|(n, o)| frees.iter().find(|f| f.0 == n && f.1 == o))
                    .map(|f| format!("Disque {} · non alloué · {}", f.0, fmt_size(f.2)))
                    .unwrap_or_else(|| "Choisir un espace libre...".into());
                egui::ComboBox::from_id_salt("pdst").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                    for f in &frees {
                        let ok = f.2 >= need;
                        ui.add_enabled_ui(ok, |ui| {
                            ui.selectable_value(&mut self.clone_pdst, Some((f.0, f.1)), format!("Disque {} · non alloué · {}{}", f.0, fmt_size(f.2), if ok { "" } else { " (trop petit)" }));
                        });
                    }
                });
                if frees.is_empty() {
                    ui.label(RichText::new("Aucun espace non alloué : réduisez ou supprimez d'abord une partition.").color(MUTED));
                }
                ui.add_space(6.0);
                if primary(ui, "Ajouter à la file d'attente", self.clone_psrc.is_some() && self.clone_pdst.is_some()) {
                    let (sn, so) = self.clone_psrc.unwrap();
                    let (dn, dof) = self.clone_pdst.unwrap();
                    acts.push(Act::Add(Op::ClonePart { src_disk: sn, src_offset: so, dst_disk: dn, dst_offset: dof }));
                    acts.push(Act::Page(Page::Partitions));
                }
            });
        }
    }

    fn page_migrate(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, "Migrer Windows", "Déplacez Windows, vos programmes et vos fichiers vers un SSD ou un nouveau disque, puis démarrez dessus.");
        let disks = self.preview.clone();
        let Some(sys) = disks.iter().find(|d| d.is_system || d.is_boot) else {
            card(ui, |ui| {
                ui.label("Le disque de Windows n'a pas été trouvé.");
            });
            return;
        };
        card(ui, |ui| {
            ui.label(RichText::new("Disque actuel de Windows").strong());
            ui.label(format!("Disque {} · {} · {} · {} à copier", sys.number, sys.model, sys.type_name(), fmt_size(sys.used_extent())));
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Choisissez le disque cible").size(15.0).strong());
        for d in disks.iter().filter(|d| d.number != sys.number) {
            let fits = d.size >= sys.used_extent() + gpt_tail_sectors(sys.sector) * sys.sector;
            let sel = self.mig_dst == Some(d.number);
            let resp = egui::Frame::none()
                .fill(if sel { ACCENT.gamma_multiply(0.08) } else { CARD })
                .rounding(10.0)
                .stroke(Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { ACCENT } else { LINE }))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Disque {}", d.number)).strong());
                        ui.label(&d.model);
                        ui.label(RichText::new(fmt_size(d.size)).color(MUTED));
                        if d.bus.contains("NVMe") || d.model.to_lowercase().contains("ssd") {
                            badge(ui, "SSD", ACCENT);
                        }
                        if !fits {
                            badge(ui, "Trop petit", DANGER);
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
        ui.checkbox(&mut self.fit, "Utiliser tout l'espace du disque cible (agrandir la dernière partition)");
        ui.label(RichText::new("Après la copie, DiskCraft reconstruit le démarrage du nouveau disque : il suffit ensuite de le choisir dans le menu de démarrage du PC.").color(MUTED));
        if let Some(t) = self.mig_dst {
            ui.label(RichText::new(format!("⚠ Tout le contenu du disque {t} sera remplacé.")).color(DANGER));
        }
        if primary(ui, "Ajouter la migration à la file d'attente", self.mig_dst.is_some()) {
            acts.push(Act::Add(Op::MigrateOs { src: sys.number, dst: self.mig_dst.unwrap(), fit: self.fit }));
            acts.push(Act::Page(Page::Partitions));
        }
    }

    fn page_convert(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, "Convertisseurs", "Changez le type d'un disque ou d'une partition sans perdre vos données.");
        let disks = self.preview.clone();
        card(ui, |ui| {
            ui.label(RichText::new("MBR / GPT").size(15.0).strong());
            ui.label(RichText::new("GPT est nécessaire pour les disques de plus de 2 To et le démarrage UEFI ; MBR pour les vieux PC en BIOS.").color(MUTED));
            disk_combo(ui, "conv", &disks, &mut self.conv_disk, |_| true);
            if let Some(d) = self.conv_disk.and_then(|n| disks.iter().find(|d| d.number == n)) {
                let n = d.number;
                ui.label(format!("Type actuel : {}", d.type_name()));
                ui.horizontal(|ui| {
                    if primary(ui, "Convertir en GPT", d.style != Style::Gpt) {
                        acts.push(Act::Add(Op::ToGpt { disk: n }));
                    }
                    if primary(ui, "Convertir en MBR", d.style != Style::Mbr) {
                        acts.push(Act::Add(Op::ToMbr { disk: n }));
                    }
                });
                if d.is_system || d.is_boot {
                    ui.label(RichText::new("Disque de Windows : la conversion en GPT utilise l'outil mbr2gpt de Windows ; passez ensuite le PC en mode UEFI.").size(12.0).color(MUTED));
                }
            }
        });
        ui.add_space(10.0);
        let letters: Vec<(char, u64)> = self.all_parts().iter().filter_map(|(_, p)| Some((p.letter?, p.free()?))).collect();
        let temp_choices: Vec<char> = letters.iter().map(|l| l.0).collect();
        let parts: Vec<(u32, Partition)> = self.all_parts().into_iter().filter(|(_, p)| p.fs == Fs::Ntfs && !p.protected()).map(|(n, p)| (n, p.clone())).collect();
        card(ui, |ui| {
            ui.label(RichText::new("NTFS en FAT32").size(15.0).strong());
            ui.label(RichText::new("FAT32 se lit partout (TV, consoles, voitures, Mac). Limites : fichiers de 4 Go maximum, partition de 2 To maximum.").color(MUTED));
            let txt = self
                .fat_part
                .and_then(|(n, o)| parts.iter().find(|(m, p)| *m == n && p.offset == o))
                .map(|(n, p)| format!("Disque {n} · {} · {}", p.title(), fmt_size(p.size)))
                .unwrap_or_else(|| "Choisir une partition NTFS...".into());
            egui::ComboBox::from_id_salt("fat").width(420.0).selected_text(txt).show_ui(ui, |ui| {
                for (n, p) in &parts {
                    ui.selectable_value(&mut self.fat_part, Some((*n, p.offset)), format!("Disque {n} · {} · {}", p.title(), fmt_size(p.size)));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Copie temporaire des fichiers sur :");
                letter_combo(ui, "tmp1", &mut self.temp_drive, &temp_choices, "choisir un lecteur");
                if let Some(t) = self.temp_drive {
                    if let Some(f) = letters.iter().find(|l| l.0 == t) {
                        ui.label(RichText::new(format!("{} libres", fmt_size(f.1))).color(MUTED));
                    }
                }
            });
            if primary(ui, "Ajouter la conversion à la file d'attente", self.fat_part.is_some() && self.temp_drive.is_some()) {
                let (n, o) = self.fat_part.unwrap();
                acts.push(Act::Add(Op::NtfsToFat32 { disk: n, offset: o, temp: self.temp_drive.unwrap() }));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Disque dynamique en disque de base").size(15.0).strong());
            ui.label(RichText::new("Les volumes simples sont copiés à part, le disque est converti, puis les volumes sont recréés avec les mêmes lettres et remplis à nouveau.").color(MUTED));
            disk_combo(ui, "dyn", &disks, &mut self.dyn_disk, |d| d.dynamic);
            if !disks.iter().any(|d| d.dynamic) {
                ui.label(RichText::new("Aucun disque dynamique sur ce PC.").color(MUTED));
            }
            ui.horizontal(|ui| {
                ui.label("Copie temporaire sur :");
                letter_combo(ui, "tmp2", &mut self.temp_drive, &temp_choices, "choisir un lecteur");
            });
            if primary(ui, "Ajouter la conversion à la file d'attente", self.dyn_disk.is_some() && self.temp_drive.is_some()) {
                acts.push(Act::Add(Op::DynamicToBasic { disk: self.dyn_disk.unwrap(), temp: self.temp_drive.unwrap() }));
            }
        });
    }

    fn page_winpe(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        heading(ui, "Disque de démarrage WinPE", "Une clé USB ou une image ISO qui démarre n'importe quel PC — même sans Windows, ou sous un vieux Windows (XP, Vista, 7, Server 2003/2008) — avec DiskCraft prêt à l'emploi.");
        let (adk, pe) = winpe::adk_ready();
        card(ui, |ui| {
            ui.label(RichText::new("1. Outils WinPE gratuits de Microsoft").strong());
            if adk && pe {
                ui.label(RichText::new("✔ Installés").color(ACCENT));
            } else {
                ui.label(RichText::new("Les outils « Windows ADK » et « WinPE » ne sont pas installés (environ 4 Go, gratuits).").color(MUTED));
                if primary(ui, "Installer les outils", true) {
                    acts.push(Act::RunNow(winpe::install_steps(), "Installation des outils WinPE".into()));
                }
            }
        });
        ui.add_space(10.0);
        let usb: Vec<char> = self.preview.iter().filter(|d| d.bus.eq_ignore_ascii_case("USB") && !d.is_system).flat_map(|d| d.parts.iter().filter_map(|p| p.letter)).collect();
        card(ui, |ui| {
            ui.label(RichText::new("2. Où écrire le disque de démarrage ?").strong());
            ui.radio_value(&mut self.pe_iso, true, "Fichier image ISO (à graver ou utiliser dans une machine virtuelle)");
            ui.radio_value(&mut self.pe_iso, false, "Clé USB (elle sera effacée)");
            if self.pe_iso {
                ui.horizontal(|ui| {
                    ui.label("Fichier :");
                    ui.add(egui::TextEdit::singleline(&mut self.pe_path).desired_width(420.0));
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label("Clé USB :");
                    letter_combo(ui, "usb", &mut self.pe_usb, &usb, "choisir");
                });
                if usb.is_empty() {
                    ui.label(RichText::new("Branchez une clé USB puis cliquez sur « Actualiser ».").color(MUTED));
                }
            }
        });
        ui.add_space(10.0);
        let ready = adk && pe && ((self.pe_iso && !self.pe_path.trim().is_empty()) || (!self.pe_iso && self.pe_usb.is_some()));
        if primary(ui, "Créer le disque de démarrage", ready) {
            let t = if self.pe_iso { winpe::Target::Iso(self.pe_path.trim().to_string()) } else { winpe::Target::Usb(self.pe_usb.unwrap()) };
            acts.push(Act::RunNow(winpe::build_steps(&t), "Création du disque de démarrage".into()));
        }
    }

    fn page_support(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Aide & support gratuit", "DiskCraft est un logiciel libre : l'aide est gratuite et ouverte à tous.");
        card(ui, |ui| {
            ui.label(RichText::new("Obtenir de l'aide").size(15.0).strong());
            ui.horizontal_wrapped(|ui| {
                if primary(ui, "Poser une question / signaler un problème", true) {
                    open_url("https://github.com/diskcraft/diskcraft/issues");
                }
                if ui.button("Guide en ligne").clicked() {
                    open_url("https://github.com/diskcraft/diskcraft/wiki");
                }
                if ui.button("Copier les informations de mes disques").clicked() {
                    let mut s = format!("DiskCraft {}\n", env!("CARGO_PKG_VERSION"));
                    for d in &self.preview {
                        s += &format!("Disque {} : {} {} {}\n", d.number, d.model, fmt_size(d.size), d.type_name());
                        for p in &d.parts {
                            s += &format!("  {} {} {} {}\n", p.title(), p.fs.name(), fmt_size(p.size), p.kind.name());
                        }
                    }
                    ui.output_mut(|o| o.copied_text = s);
                }
            });
            ui.label(RichText::new("Joignez les informations copiées et le journal de l'opération : la communauté vous répond gratuitement.").color(MUTED).size(12.0));
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Questions fréquentes").size(15.0).strong());
            let faq = [
                ("Mes données sont-elles en sécurité ?", "Rien n'est modifié avant que vous cliquiez sur « Exécuter ». Les opérations qui gardent les données (redimensionner, fusionner, convertir, cloner) sont conçues pour ne rien perdre, mais une coupure de courant pendant l'opération peut tout casser : faites toujours une sauvegarde avant."),
                ("Pourquoi Windows demande-t-il une autorisation ?", "Modifier un disque demande les droits d'administrateur. DiskCraft lance alors un petit assistant qui exécute la file d'attente et écrit un journal."),
                ("Je ne peux pas agrandir C: : pourquoi ?", "Il faut un espace non alloué juste après C:. Souvent une partition de récupération est placée entre les deux : supprimez ou déplacez la partition voisine, puis agrandissez C:."),
                ("Mon nouveau SSD est plus petit que mon disque dur.", "Réduisez d'abord la dernière partition du disque actuel (Redimensionner) pour que tout tienne, puis lancez la migration."),
                ("Après la migration, le PC démarre toujours sur l'ancien disque.", "Ouvrez le menu de démarrage du PC (souvent F12, F8 ou Échap au démarrage) et choisissez le nouveau disque, ou débranchez l'ancien."),
                ("Quelles versions de Windows sont prises en charge ?", "DiskCraft fonctionne sous Windows 10 et 11 et Windows Server 2016, 2019, 2022 et 2025. Pour un PC sous un Windows plus ancien, créez le disque de démarrage WinPE : il démarre n'importe quel PC."),
            ];
            for (q, a) in faq {
                egui::CollapsingHeader::new(RichText::new(q).strong()).show(ui, |ui| {
                    ui.label(a);
                });
            }
        });
    }

    fn page_about(&mut self, ui: &mut egui::Ui) {
        heading(ui, "DiskCraft", &format!("Version {} — gestionnaire de partitions libre et gratuit", env!("CARGO_PKG_VERSION")));
        card(ui, |ui| {
            ui.label(RichText::new("Fonctions").size(15.0).strong());
            for f in [
                "Redimensionner, fusionner, créer, formater, supprimer, effacer et vérifier les partitions",
                "Cloner des disques et des partitions (copie à chaud grâce aux instantanés)",
                "Migrer Windows vers un SSD ou un disque dur, avec réparation du démarrage",
                "Convertir MBR en GPT et GPT en MBR sans perte de données",
                "Convertir NTFS en FAT32, formater en FAT32 au-delà de 32 Go",
                "Transformer un disque dynamique en disque de base",
                "Créer un disque de démarrage WinPE",
                "Support technique gratuit par la communauté",
            ] {
                ui.label(format!("✔  {f}"));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Compatibilité").size(15.0).strong());
            ui.label("Windows 11, Windows 10 · Windows Server 2025, 2022, 2019, 2016");
            ui.label(RichText::new("Windows plus anciens (8.1, 8, 7, Vista, XP, Server 2012/2008/2003, Home Server) : utilisez le disque de démarrage WinPE, qui démarre ces PC et gère leurs disques.").color(MUTED));
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Licence").size(15.0).strong());
            ui.label("Logiciel libre sous licence GNU GPL version 3 ou ultérieure : vous pouvez l'utiliser, l'étudier, le modifier et le partager librement, pour toujours, gratuitement.");
        });
    }

    // ---------- dialogs ----------

    fn dialogs(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let Some(mut dlg) = self.dialog.take() else { return };
        let mut open = true;
        let mut close = false;
        let title = match &dlg {
            Dlg::Resize { .. } => "Redimensionner / déplacer la partition".to_string(),
            Dlg::Create { .. } => "Créer une partition".to_string(),
            Dlg::Format { .. } => "Formater la partition".to_string(),
            Dlg::Delete { .. } => "Supprimer la partition".to_string(),
            Dlg::Wipe { offset: None, .. } => "Effacer le disque".to_string(),
            Dlg::Wipe { .. } => "Effacer la partition".to_string(),
            Dlg::Merge { .. } => "Fusionner des partitions".to_string(),
            Dlg::Letter { .. } => "Lettre de lecteur".to_string(),
            Dlg::Label { .. } => "Nom de la partition".to_string(),
            Dlg::Check { .. } => "Vérifier la partition".to_string(),
            Dlg::Confirm { .. } => "Exécuter les opérations".to_string(),
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
                    "Faites glisser la partition (en bleu) sur la carte du disque pour la poser où vous voulez dans un espace libre, puis réglez sa taille en dessous."
                } else {
                    "Faites glisser les bords de la barre du bas pour changer la taille de la partition."
                }).color(MUTED));
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
                            pc.text(r.left_top() + Vec2::new(6.0, 12.0), Align2::LEFT_TOP, p.title(), FontId::proportional(12.0), TEXT);
                            pc.text(r.left_top() + Vec2::new(6.0, 30.0), Align2::LEFT_TOP, fmt_size(p.size), FontId::proportional(11.0), MUTED);
                        }
                        None => {
                            painter.rect_filled(r, 5.0, Color32::from_rgb(0x2c, 0x2e, 0x34));
                            let pc = painter.with_clip_rect(r);
                            let mut xx = r.left() - h;
                            while xx < r.right() {
                                pc.line_segment([egui::pos2(xx, r.bottom()), egui::pos2(xx + h, r.top())], Stroke::new(1.0_f32, Color32::from_rgb(0x42, 0x45, 0x4d)));
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
                painter.rect_stroke(sel, 5.0, Stroke::new(2.0_f32, ACCENT_DARK));
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
                    ui.label(RichText::new(format!("Disque {} · {} · {}", dk.number, fmt_size(dk.size), dk.type_name())).size(12.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(*can_resize, egui::Button::new("Remplir tout l'espace non alloué").rounding(6.0)).clicked() {
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
                ui.label(RichText::new(format!("Espace où se trouve la partition : {}", fmt_size(re - rs))).strong());
                let zw = 640.0f32;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(zw, 46.0), Sense::hover());
                let px_per_mb = zw / total_mb.max(1.0) as f32;
                let x0 = rect.left() + start_mb as f32 * px_per_mb;
                let x1 = x0 + (*size_mb as f32 * px_per_mb).max(16.0);
                let part = egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1.min(rect.right()), rect.bottom()));
                let p = ui.painter();
                p.rect_filled(rect, 6.0, Color32::from_rgb(0x2c, 0x2e, 0x34));
                let mut xx = rect.left() - 46.0;
                while xx < rect.right() {
                    p.with_clip_rect(rect).line_segment([egui::pos2(xx, rect.bottom()), egui::pos2(xx + 46.0, rect.top())], Stroke::new(1.0_f32, Color32::from_rgb(0x42, 0x45, 0x4d)));
                    xx += 8.0;
                }
                p.rect_filled(part, 6.0, Color32::from_rgb(0x2f, 0x7f, 0xd8).gamma_multiply(0.85));
                p.rect_stroke(part, 6.0, Stroke::new(2.0_f32, ACCENT_DARK));
                for hx in [part.left() + 1.0, part.right() - 1.0] {
                    p.circle_filled(egui::pos2(hx, part.center().y), 7.0, ACCENT_DARK);
                    p.text(egui::pos2(hx, part.center().y), Align2::CENTER_CENTER, "‖", FontId::proportional(10.0), NAV);
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
                    ui.label("Espace non alloué avant :");
                    ui.add_enabled(*can_move, egui::DragValue::new(&mut start_mb).range(0.0..=(total_mb - *size_mb).max(0.0)).speed(16.0).suffix(" Mo"));
                    ui.label(RichText::new(fmt_size((start_mb as u64) * MIB)).color(MUTED));
                    ui.end_row();
                    ui.label("Taille de la partition :");
                    ui.add_enabled(*can_resize, egui::DragValue::new(size_mb).range(min_mb..=(total_mb - start_mb).min(max_mb).max(min_mb)).speed(16.0).suffix(" Mo"));
                    ui.label(RichText::new(fmt_size((*size_mb as u64) * MIB)).color(MUTED));
                    ui.end_row();
                    let mut after = (total_mb - start_mb - *size_mb).max(0.0);
                    ui.label("Espace non alloué après :");
                    if ui.add_enabled(*can_resize || *can_move, egui::DragValue::new(&mut after).range(0.0..=(total_mb - start_mb - min_mb).max(0.0)).speed(16.0).suffix(" Mo")).changed() {
                        if *can_resize {
                            *size_mb = (total_mb - start_mb - after).clamp(min_mb, max_mb);
                        } else {
                            start_mb = (total_mb - *size_mb - after).clamp(0.0, (total_mb - *size_mb).max(0.0));
                        }
                    }
                    ui.label(RichText::new(fmt_size((after as u64) * MIB)).color(MUTED));
                    ui.end_row();
                });
                *size_mb = size_mb.round();
                start_mb = start_mb.round();
                if *can_move {
                    *pos = (rs + (start_mb as u64) * MIB).min(re.saturating_sub(size_of(*size_mb))).max(rs);
                }
                if !*can_resize {
                    ui.label(RichText::new("Ce système de fichiers peut être déplacé mais pas redimensionné (seul NTFS se redimensionne).").size(12.0).color(MUTED));
                }
                if !*can_move {
                    ui.label(RichText::new("Windows utilise cette partition : elle peut changer de taille mais pas de place. Pour la déplacer, démarrez sur le disque de démarrage DiskCraft.").size(12.0).color(MUTED));
                }
                let moving = *pos != *offset;
                if moving {
                    ui.label(RichText::new("⚠ Déplacer recopie toutes les données de la partition : comptez plusieurs minutes et ne coupez surtout pas le courant.").color(PENDING));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if primary(ui, "OK", true) {
                        let new_size = size_of(*size_mb);
                        acts.push(Act::Add(Op::Resize { disk: *disk, offset: *offset, new_offset: *pos, new_size }));
                        close = true;
                    }
                    if ui.add(egui::Button::new("Annuler").rounding(6.0)).clicked() {
                        close = true;
                    }
                });
            }
            Dlg::Create { disk, offset, max, size_mb, fs, label, letter } => {
                let maxm = (*max / MIB) as f64;
                ui.add(egui::Slider::new(size_mb, 1.0..=maxm.max(1.0)).text("Mo").step_by(1.0));
                fs_choice(ui, fs);
                ui.horizontal(|ui| {
                    ui.label("Nom :");
                    ui.text_edit_singleline(label);
                });
                ui.horizontal(|ui| {
                    ui.label("Lettre :");
                    letter_combo(ui, "newl", letter, &free_letters, "aucune");
                });
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::Create { disk: *disk, offset: *offset, size: (*size_mb as u64 * MIB).min(*max), fs: fs.clone(), label: label.clone(), letter: *letter }));
                    close = true;
                }
            }
            Dlg::Format { disk, offset, fs, label, cluster, quick } => {
                fs_choice(ui, fs);
                ui.horizontal(|ui| {
                    ui.label("Nom :");
                    ui.text_edit_singleline(label);
                });
                ui.horizontal(|ui| {
                    ui.label("Taille de cluster :");
                    let txt = if *cluster == 0 { "Par défaut".to_string() } else { format!("{} Ko", *cluster as f32 / 1024.0) };
                    egui::ComboBox::from_id_salt("clu").selected_text(txt).show_ui(ui, |ui| {
                        ui.selectable_value(cluster, 0, "Par défaut");
                        for c in [512u32, 1024, 2048, 4096, 8192, 16384, 32768, 65536] {
                            ui.selectable_value(cluster, c, format!("{} Ko", c as f32 / 1024.0));
                        }
                    });
                });
                ui.checkbox(quick, "Formatage rapide");
                ui.label(RichText::new("⚠ Toutes les données de la partition seront perdues.").color(DANGER));
                if primary(ui, "OK", true) {
                    acts.push(Act::Add(Op::Format { disk: *disk, offset: *offset, fs: fs.clone(), label: label.clone(), cluster: *cluster, quick: *quick }));
                    close = true;
                }
            }
            Dlg::Delete { disk, offset } => {
                ui.label("La partition deviendra de l'espace non alloué.");
                ui.label(RichText::new("⚠ Toutes ses données seront perdues.").color(DANGER));
                if primary(ui, "Supprimer", true) {
                    acts.push(Act::Add(Op::Delete { disk: *disk, offset: *offset }));
                    close = true;
                }
            }
            Dlg::Wipe { disk, offset, passes } => {
                ui.label("Les données sont écrasées et ne pourront plus être récupérées, même avec un logiciel spécialisé.");
                ui.add(egui::Slider::new(passes, 1..=10).text("passe(s)"));
                ui.label(RichText::new("Chaque passe sauf la dernière écrit des données aléatoires ; la dernière écrit des zéros.").size(12.0).color(MUTED));
                if primary(ui, "Effacer", true) {
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
                        ui.label(format!("{} et {} deviendront une seule partition de {}.", d.parts[i].title(), d.parts[j].title(), fmt_size(d.parts[j].end() - d.parts[i].offset)));
                        if let Some(l) = d.parts[j].letter {
                            ui.label(format!("Le contenu de {l}: sera copié dans le dossier « Contenu de {l} » de la première partition."));
                        }
                    }
                }
                if primary(ui, "Fusionner", true) {
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
                    ui.label("Lettre :");
                    letter_combo(ui, "letter", letter, &choices, "aucune (masquer)");
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
                ui.radio_value(mode, 0, "Rechercher les erreurs (sans rien modifier)");
                ui.radio_value(mode, 1, "Rechercher et réparer les erreurs");
                ui.radio_value(mode, 2, "Test de surface (lire chaque secteur)");
                ui.label(RichText::new("La vérification se lance tout de suite ; elle ne passe pas par la file d'attente.").size(12.0).color(MUTED));
                if primary(ui, "Lancer", true) {
                    if let Some(d) = preview.iter().find(|d| d.number == *disk) {
                        if let Some(i) = d.part_at(*offset) {
                            acts.push(Act::RunNow(ops::check_steps(d, &d.parts[i], *mode), "Vérification de la partition".into()));
                        }
                    }
                    close = true;
                }
            }
            Dlg::Confirm { backup_ok } => {
                ui.label("Ces opérations vont être appliquées à vos disques :");
                for (i, o) in ops_list.iter().enumerate() {
                    ui.label(format!("{}. {o}", i + 1));
                }
                let ok = matches!(plan, Some(Ok(_)));
                match &plan {
                    Some(Ok(steps)) => {
                        egui::CollapsingHeader::new(format!("Voir les {} étapes détaillées", steps.len())).show(ui, |ui| {
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
                ui.checkbox(backup_ok, "J'ai sauvegardé mes fichiers importants et le PC restera branché");
                ui.label(RichText::new("Ne coupez pas le courant pendant l'opération.").size(12.0).color(MUTED));
                if primary(ui, "Exécuter maintenant", *backup_ok && ok) {
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
                    ui.label(RichText::new("Ne coupez pas le courant et ne débranchez pas les disques.").color(MUTED).size(12.0));
                }
                Some(true) => {
                    ui.label(RichText::new("✔ Terminé avec succès").size(16.0).color(ACCENT).strong());
                }
                Some(false) => {
                    ui.label(RichText::new("⚠ L'opération s'est arrêtée").size(16.0).color(DANGER).strong());
                }
            }
            egui::ScrollArea::vertical().max_height(200.0).stick_to_bottom(true).show(ui, |ui| {
                for l in &r.lines {
                    ui.label(RichText::new(l).monospace().size(11.5));
                }
            });
            if r.done.is_some() {
                ui.horizontal(|ui| {
                    if primary(ui, "Fermer", true) {
                        acts.push(Act::Reload);
                    }
                    if ui.button("Ouvrir le journal").clicked() {
                        open_url(&r.log.display().to_string());
                    }
                });
            }
        });
    }
}

fn fs_choice(ui: &mut egui::Ui, fs: &mut Fs) {
    ui.horizontal(|ui| {
        ui.label("Système de fichiers :");
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
