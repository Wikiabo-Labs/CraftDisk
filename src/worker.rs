//! The elevated helper: `DiskCraft.exe --worker plan.json log.txt` carries out a plan
//! and writes its progress to the log file, which the window reads.

use crate::plan::{DiskId, Plan, SrcPart, Step};
use crate::rawio::{seed, Dev, XorShift};
use std::io::{BufRead, BufReader, Read, Write};

const CHUNK: u64 = 4 * 1024 * 1024;

pub const PS_PRELUDE: &str = r#"[Console]::OutputEncoding = [Text.Encoding]::UTF8
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
function P($d, $o) {
  $x = Get-Partition -DiskNumber $d | Where-Object { $_.Offset -eq [uint64]$o }
  if (-not $x) { throw "Partition introuvable (disque $d, position $o)" }
  $x
}
"#;

pub struct Log {
    file: std::fs::File,
    last_pct: f32,
}

impl Log {
    pub fn line(&mut self, s: &str) {
        let _ = writeln!(self.file, "{s}");
        let _ = self.file.flush();
    }
    pub fn info(&mut self, s: &str) {
        for l in s.lines() {
            let l = l.trim_end();
            if !l.is_empty() {
                self.line(&format!("INFO {l}"));
            }
        }
    }
    pub fn pct(&mut self, p: f32) {
        if (p - self.last_pct).abs() >= 0.002 || p >= 1.0 || p == 0.0 {
            self.last_pct = p;
            self.line(&format!("PCT {p:.4}"));
        }
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Runs a PowerShell script and sends each output line to `on_line`. Returns whether it succeeded.
/// The script goes to PowerShell on the command line (-EncodedCommand), never through a file:
/// a file in a shared folder could be swapped between its creation and its run by the elevated worker.
pub fn run_ps_file(script: &str, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
    use base64::Engine;
    let body = format!(
        "{PS_PRELUDE}try {{\n{script}\n}} catch {{\n  Write-Output (\"ERREUR: \" + $_.Exception.Message)\n  exit 1\n}}\nexit 0\n"
    );
    let utf16: Vec<u8> = body.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    if encoded.len() > 30_000 {
        return Err(crate::tr!("Le script PowerShell est trop long.", "The PowerShell script is too long.").into());
    }
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded]);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| crate::trf!("PowerShell introuvable : {e}", "PowerShell not found: {e}"))?;
    let (Some(stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        return Err(crate::tr!("Sortie de PowerShell inaccessible.", "PowerShell output is not available.").into());
    };
    // stderr must be read all the time, or PowerShell blocks when its pipe fills up. Keep only the tail.
    let err_reader = std::thread::spawn(move || {
        let mut tail: Vec<u8> = Vec::new();
        let mut buf = [0u8; 8192];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&buf[..n]);
            if tail.len() > 4096 {
                tail.drain(..tail.len() - 4096);
            }
        }
        String::from_utf8_lossy(&tail).trim().to_string()
    });
    let mut err_text = String::new();
    let mut reader = BufReader::new(stdout);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match reader.read_until(b'\n', &mut raw) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(e) = line.strip_prefix("ERREUR: ") {
            err_text = e.to_string();
        }
        on_line(line);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr_tail = err_reader.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else if !err_text.is_empty() {
        Err(err_text)
    } else if !stderr_tail.is_empty() {
        Err(crate::trf!("La commande a échoué (code {:?}) : {stderr_tail}", "The command failed (code {:?}): {stderr_tail}", status.code()))
    } else {
        Err(crate::trf!("La commande a échoué (code {:?}).", "The command failed (code {:?}).", status.code()))
    }
}

fn ps_value(script: &str) -> Result<String, String> {
    let mut last = String::new();
    run_ps_file(script, &mut |l| {
        if !l.trim().is_empty() {
            last = l.trim().to_string();
        }
    })?;
    Ok(last)
}

fn volume_path(disk: u32, offset: u64) -> Result<String, String> {
    let v = ps_value(&format!(
        "$p = P {disk} {offset}\n$a = @($p.AccessPaths) | Where-Object {{ $_ -like '\\\\?\\Volume*' }} | Select-Object -First 1\nif (-not $a) {{ throw 'Volume introuvable' }}\nWrite-Output $a"
    ))?;
    Ok(v.trim_end_matches('\\').to_string())
}

fn sector_size(disk: u32) -> u64 {
    ps_value(&format!("(Get-Disk -Number {disk}).LogicalSectorSize")).ok().and_then(|s| s.parse().ok()).filter(|&s: &u64| s >= 512).unwrap_or(512)
}

struct Shadow {
    id: String,
    dev: Dev,
    len: u64,
}

fn make_shadow(letter: char, log: &mut Log) -> Option<Shadow> {
    let r = ps_value(&format!(
        "$r = (Get-WmiObject -List Win32_ShadowCopy).Create('{letter}:\\', 'ClientAccessible')\nif ($r.ReturnValue -ne 0) {{ throw \"code $($r.ReturnValue)\" }}\n$s = Get-WmiObject Win32_ShadowCopy | Where-Object {{ $_.ID -eq $r.ShadowID }}\nWrite-Output ($s.DeviceObject + '|' + $s.ID)"
    ));
    match r {
        Ok(v) => {
            let (path, id) = v.split_once('|')?;
            let dev = Dev::open(path, false).ok()?;
            let len = dev.len().unwrap_or(0);
            log.info(&crate::trf!("Instantané de {letter}: créé : les fichiers ouverts sont copiés dans un état cohérent.", "Snapshot of {letter}: created: open files are copied in a consistent state."));
            Some(Shadow { id: id.to_string(), dev, len })
        }
        Err(e) => {
            log.info(&crate::trf!("Instantané de {letter}: impossible ({e}) : copie directe.", "Snapshot of {letter}: not possible ({e}): direct copy."));
            None
        }
    }
}

fn drop_shadow(s: &Shadow) {
    let _ = ps_value(&format!("Get-WmiObject Win32_ShadowCopy | Where-Object {{ $_.ID -eq '{}' }} | ForEach-Object {{ $_.Delete() }}", s.id));
}

fn clone_disk(src: u32, dst: u32, ss: u64, parts: &[SrcPart], extent: u64, log: &mut Log) -> Result<(), String> {
    let mut sdev = Dev::disk(src, false)?;
    let mut ddev = Dev::disk(dst, true)?;
    if ddev.len()? < extent {
        return Err(crate::tr!("Le disque cible est trop petit.", "The target disk is too small.").into());
    }
    let mut shadows: Vec<(SrcPart, Shadow)> = Vec::new();
    for p in parts {
        if let (Some(l), true) = (p.letter, p.ntfs) {
            if let Some(s) = make_shadow(l, log) {
                shadows.push((p.clone(), s));
            }
        }
    }
    let end = extent.div_ceil(ss) * ss;
    let mut buf = vec![0u8; CHUNK as usize];
    let mut pos = 0u64;
    let result = (|| -> Result<(), String> {
        while pos < end {
            let chunk_end = (pos + CHUNK).min(end);
            // Pick the source for this piece: a snapshot for mounted NTFS volumes, else the disk.
            let mut len = chunk_end - pos;
            let mut from_shadow = None;
            for (i, (p, s)) in shadows.iter().enumerate() {
                if pos >= p.offset && pos < p.offset + p.size {
                    let rel = pos - p.offset;
                    if rel < s.len {
                        len = len.min(p.offset + p.size - pos).min(s.len - rel);
                        from_shadow = Some((i, rel));
                    } else {
                        len = len.min(p.offset + p.size - pos);
                    }
                } else if p.offset > pos {
                    len = len.min(p.offset - pos);
                }
            }
            let b = &mut buf[..len as usize];
            match from_shadow {
                Some((i, rel)) => shadows[i].1.dev.read_at(rel, b)?,
                None => sdev.read_at(pos, b)?,
            }
            ddev.write_at(pos, b)?;
            pos += len;
            log.pct(pos as f32 / end as f32);
        }
        Ok(())
    })();
    for (_, s) in &shadows {
        drop_shadow(s);
    }
    result
}

fn clone_part(src_disk: u32, part: &SrcPart, dst_disk: u32, dst_offset: u64, log: &mut Log) -> Result<(), String> {
    let shadow = match (part.letter, part.ntfs) {
        (Some(l), true) => make_shadow(l, log),
        _ => None,
    };
    let mut sdisk = Dev::disk(src_disk, false)?;
    let path = volume_path(dst_disk, dst_offset)?;
    let mut ddev = Dev::open(&path, true)?;
    ddev.lock_dismount()?;
    let total = part.size.min(ddev.len().unwrap_or(part.size));
    let mut buf = vec![0u8; CHUNK as usize];
    let mut pos = 0u64;
    let mut shadow = shadow;
    let r = (|| -> Result<(), String> {
        while pos < total {
            let len = CHUNK.min(total - pos);
            let b = &mut buf[..len as usize];
            match shadow.as_mut() {
                Some(s) if pos + len <= s.len => s.dev.read_at(pos, b)?,
                _ => sdisk.read_at(part.offset + pos, b)?,
            }
            ddev.write_at(pos, b)?;
            pos += len;
            log.pct(pos as f32 / total as f32);
        }
        Ok(())
    })();
    if let Some(s) = &shadow {
        drop_shadow(s);
    }
    r
}

fn overwrite(dev: &mut Dev, start: u64, len: u64, passes: u8, log: &mut Log) -> Result<(), String> {
    let mut rng = XorShift(seed());
    let mut buf = vec![0u8; CHUNK as usize];
    for k in 0..passes {
        let last = k + 1 == passes;
        log.info(&crate::trf!("Passe {} sur {passes} ({})", "Pass {} of {passes} ({})", k + 1, if last { crate::tr!("zéros", "zeros") } else { crate::tr!("données aléatoires", "random data") }));
        let mut pos = 0u64;
        while pos < len {
            let n = CHUNK.min(len - pos);
            let b = &mut buf[..n as usize];
            if last {
                b.fill(0);
            } else {
                rng.fill(b);
            }
            dev.write_at(start + pos, b)?;
            pos += n;
            log.pct((k as f32 + pos as f32 / len as f32) / passes as f32);
        }
    }
    Ok(())
}

/// Move a partition's data inside the same disk. Moving left copies from the start,
/// moving right copies from the end, so overlapping ranges are never overwritten too early.
fn move_part(disk: u32, offset: u64, size: u64, new_offset: u64, ss: u64, log: &mut Log) -> Result<(), String> {
    let size = size / ss * ss;
    // Take the volume away from Windows while its data moves (kept until the table is updated).
    let vol = match volume_path(disk, offset) {
        Ok(p) => {
            let v = Dev::open(&p, true)?;
            v.lock_dismount()?;
            Some(v)
        }
        Err(_) => None,
    };
    log.info(crate::tr!("Déplacement en cours : ne coupez surtout pas le courant avant la fin.", "Moving: do not cut the power before it finishes."));
    let mut d = Dev::disk(disk, true)?;
    let mut buf = vec![0u8; CHUNK as usize];
    let mut done = 0u64;
    while done < size {
        let n = CHUNK.min(size - done);
        let rel = if new_offset < offset { done } else { size - done - n };
        let b = &mut buf[..n as usize];
        d.read_at(offset + rel, b)?;
        d.write_at(new_offset + rel, b)?;
        done += n;
        log.pct(done as f32 / size as f32 * 0.98);
    }
    crate::gpt::fix_boot_offsets(&mut d, new_offset, size, ss)?;
    crate::gpt::move_entry(disk, ss, offset / ss, new_offset / ss)?;
    drop(vol);
    log.pct(1.0);
    log.info(crate::tr!("Partition déplacée.", "Partition moved."));
    Ok(())
}

fn surface_test(disk: u32, offset: u64, size: u64, ss: u64, log: &mut Log) -> Result<(), String> {
    let mut dev = Dev::disk(disk, false)?;
    let mut buf = vec![0u8; CHUNK as usize];
    let mut pos = 0u64;
    let mut bad = 0u64;
    let total = size / ss * ss;
    while pos < total {
        let n = CHUNK.min(total - pos);
        if dev.read_at(offset + pos, &mut buf[..n as usize]).is_err() {
            // Re-read sector by sector to find the bad ones.
            let mut s = 0;
            while s < n {
                if dev.read_at(offset + pos + s, &mut buf[..ss as usize]).is_err() {
                    bad += 1;
                    if bad <= 200 {
                        log.info(&crate::trf!("Secteur illisible : {}", "Unreadable sector: {}", (offset + pos + s) / ss));
                    }
                }
                s += ss;
            }
        }
        pos += n;
        log.pct(pos as f32 / total as f32);
    }
    if bad == 0 {
        log.info(crate::tr!("Test de surface terminé : aucun secteur défectueux.", "Surface test finished: no bad sectors."));
    } else {
        log.info(&crate::trf!("Test de surface terminé : {bad} secteur(s) défectueux. Sauvegardez vos données.", "Surface test finished: {bad} bad sector(s). Back up your data."));
    }
    Ok(())
}

fn fix_boot(disk: u32, win: u64, sys: u64, uefi: bool, log: &mut Log) -> Result<(), String> {
    let fw = if uefi { "UEFI" } else { "BIOS" };
    let script = format!(
        r#"# Réparation du démarrage
$w = P {disk} {win}
$s = P {disk} {sys}
$free = [char[]](70..90) | Where-Object {{ -not (Test-Path "$($_):\") }}
$dp = Join-Path $env:TEMP 'diskcraft_boot.txt'
$W = [string]$w.DriveLetter
if ($W -eq [string][char]0 -or $W -eq '') {{ $W = [string]$free[0]; "select disk {disk}`nselect partition $($w.PartitionNumber)`nassign letter=$W" | Set-Content -Encoding ASCII $dp; diskpart /s $dp | Out-Null }}
$S = $W
$tempS = $false
if ($s.PartitionNumber -ne $w.PartitionNumber) {{
  $S = [string]$free[1]; $tempS = $true
  "select disk {disk}`nselect partition $($s.PartitionNumber)`nassign letter=$S" | Set-Content -Encoding ASCII $dp; diskpart /s $dp | Out-Null
}}
Write-Output "bcdboot ${{W}}:\Windows /s ${{S}}: /f {fw}"
bcdboot "${{W}}:\Windows" /s "${{S}}:" /f {fw}
if ($LASTEXITCODE -ne 0) {{ throw 'bcdboot a échoué.' }}
if ('{fw}' -eq 'BIOS') {{
  $bs = Get-Command bootsect.exe -ErrorAction SilentlyContinue
  if ($bs) {{ bootsect.exe /nt60 "${{S}}:" /mbr }} else {{ Write-Output 'bootsect introuvable : le code de démarrage copié du disque source est conservé.' }}
}}
if ($tempS) {{ "select disk {disk}`nselect partition $($s.PartitionNumber)`nremove letter=$S" | Set-Content -Encoding ASCII $dp; diskpart /s $dp | Out-Null }}
Remove-Item $dp -ErrorAction SilentlyContinue
Write-Output 'Démarrage réparé : choisissez le nouveau disque dans le menu de démarrage du PC.'"#
    );
    run_ps_file(&script, &mut |l| log.info(l))
}

fn run_step(step: &Step, log: &mut Log) -> Result<(), String> {
    match step {
        Step::Ps { script } => run_ps_file(script, &mut |l| {
            if !l.starts_with("ERREUR: ") {
                log.info(l)
            }
        }),
        Step::CloneDisk { src, dst, sector, parts, extent } => clone_disk(*src, *dst, *sector, parts, *extent, log),
        Step::ClonePart { src_disk, part, dst_disk, dst_offset } => clone_part(*src_disk, part, *dst_disk, *dst_offset, log),
        Step::FixIds { disk, sector } => crate::gpt::fix_ids(*disk, *sector),
        Step::MovePart { disk, offset, size, new_offset, sector } => move_part(*disk, *offset, *size, *new_offset, *sector, log),
        Step::FixBoot { disk, win_offset, sys_offset, uefi } => fix_boot(*disk, *win_offset, *sys_offset, *uefi, log),
        Step::WipeDisk { disk, passes } => {
            let mut d = Dev::disk(*disk, true)?;
            let len = d.len()?;
            overwrite(&mut d, 0, len, *passes, log)
        }
        Step::WipePart { disk, offset, passes } => {
            let path = volume_path(*disk, *offset)?;
            let mut d = Dev::open(&path, true)?;
            d.lock_dismount()?;
            let len = d.len()?;
            overwrite(&mut d, 0, len, *passes, log)
        }
        Step::ToGpt { disk, sector } => crate::gpt::mbr_to_gpt(*disk, *sector),
        Step::ToMbr { disk, sector, fat32_offsets } => crate::gpt::gpt_to_mbr(*disk, *sector, fat32_offsets),
        Step::FormatFat32 { disk, offset, label, cluster } => {
            let ss = sector_size(*disk);
            let path = volume_path(*disk, *offset)?;
            let mut d = Dev::open(&path, true)?;
            d.lock_dismount()?;
            let size = d.len()?;
            crate::fat32::format(&mut d, size, ss, offset / ss, label, *cluster, &mut |p| log.pct(p))
        }
        Step::SurfaceTest { disk, offset, size, sector } => surface_test(*disk, *offset, *size, *sector, log),
    }
}

struct LiveDisk {
    number: u32,
    size: u64,
    uid: String,
    model: String,
    system: bool,
}

fn live_disks() -> Result<Vec<LiveDisk>, String> {
    let mut out = Vec::new();
    run_ps_file(
        "Get-Disk | ForEach-Object { \"DISK|$($_.Number)|$($_.Size)|$([int]($_.IsSystem -or $_.IsBoot))|$($_.UniqueId)|$($_.FriendlyName)\" }",
        &mut |l| {
            let mut f = l.trim().splitn(6, '|');
            if f.next() != Some("DISK") {
                return;
            }
            if let (Some(n), Some(sz), Some(sys), Some(uid), Some(model)) = (f.next(), f.next(), f.next(), f.next(), f.next()) {
                if let (Ok(number), Ok(size)) = (n.parse(), sz.parse()) {
                    out.push(LiveDisk { number, size, uid: uid.trim().to_string(), model: model.trim().to_string(), system: sys == "1" });
                }
            }
        },
    )?;
    Ok(out)
}

/// Every disk the plan was made for must still be the same one under the same number.
fn check_disks(expected: &[DiskId], live: &[LiveDisk]) -> Result<(), String> {
    if expected.is_empty() {
        return Err(crate::tr!("Le plan ne contient aucune identité de disque : exécution refusée.", "The plan contains no disk identity: execution refused.").into());
    }
    for e in expected {
        match live.iter().find(|d| d.number == e.number) {
            None => return Err(crate::trf!("Le disque {} n'existe plus : relancez DiskCraft et recommencez.", "Disk {} no longer exists: restart DiskCraft and try again.", e.number)),
            Some(d) if d.size != e.size || d.uid != e.uid || d.model != e.model => {
                return Err(crate::trf!("Le disque {} n'est plus le même qu'à l'aperçu (débranché, rebranché ou renuméroté) : rien n'a été modifié. Relancez DiskCraft.", "Disk {} is no longer the same as in the preview (unplugged, plugged back in or renumbered): nothing was changed. Restart DiskCraft.",
                    e.number
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Steps that destroy a whole disk must never reach the disk Windows runs on, whatever the plan says:
/// checked against the live state before any step runs.
fn check_destructive(steps: &[Step], live: &[LiveDisk]) -> Result<(), String> {
    for s in steps {
        let target = match s {
            Step::WipeDisk { disk, .. } | Step::ToMbr { disk, .. } => Some(*disk),
            Step::CloneDisk { dst, .. } => Some(*dst),
            _ => None,
        };
        if let Some(n) = target {
            if live.iter().find(|d| d.number == n).map_or(true, |d| d.system) {
                return Err(crate::trf!("Le disque {n} est le disque de Windows (ou n'existe plus) : opération refusée.", "Disk {n} is the Windows disk (or no longer exists): operation refused."));
            }
        }
    }
    Ok(())
}

/// Opens the journal without following links: the worker is elevated, the file lives in a folder
/// the person can write to.
fn open_log(path: &str) -> Option<std::fs::File> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut o = std::fs::OpenOptions::new();
    o.append(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    o.open(path).ok()
}

fn load_plan(plan_path: &str, expected_hash: &str) -> Result<Plan, String> {
    let bytes = std::fs::read(plan_path).map_err(|e| crate::trf!("Plan illisible : {e}", "Unreadable plan: {e}"))?;
    if !sha256_hex(&bytes).eq_ignore_ascii_case(expected_hash.trim()) {
        return Err(crate::tr!("Le plan a été modifié après votre autorisation : exécution refusée.", "The plan was changed after your permission: execution refused.").into());
    }
    serde_json::from_slice(&bytes).map_err(|e| crate::trf!("Plan illisible : {e}", "Unreadable plan: {e}"))
}

pub fn run(plan_path: &str, log_path: &str, plan_hash: &str) -> i32 {
    let Some(file) = open_log(log_path) else { return 2 };
    let mut log = Log { file, last_pct: -1.0 };
    fn fail(log: &mut Log, e: &str) -> i32 {
        log.line(&format!("ERREUR {e}"));
        log.line("FIN ECHEC");
        1
    }
    let plan = match load_plan(plan_path, plan_hash) {
        Ok(p) => p,
        Err(e) => return fail(&mut log, &e),
    };
    let live = match live_disks() {
        Ok(l) => l,
        Err(e) => return fail(&mut log, &crate::trf!("Impossible de relire les disques : {e}", "Could not read the disks again: {e}")),
    };
    if let Err(e) = check_disks(&plan.disks, &live).and_then(|_| check_destructive(&plan.steps, &live)) {
        return fail(&mut log, &e);
    }
    let n = plan.steps.len();
    for (i, step) in plan.steps.iter().enumerate() {
        log.line(&format!("ETAPE {}/{} {}", i + 1, n, step.title()));
        log.pct(0.0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_step(step, &mut log))).unwrap_or_else(|_| Err(crate::tr!("Erreur interne inattendue.", "Unexpected internal error.").into()));
        if let Err(e) = r {
            return fail(&mut log, &e);
        }
        log.pct(1.0);
    }
    log.line("FIN OK");
    0
}
