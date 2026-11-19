//! Reads the computer's disks (read-only) through the Windows storage cmdlets.

use crate::model::*;
use serde::Deserialize;

const SCRIPT: &str = r#"
$ErrorActionPreference='SilentlyContinue'
$out = foreach($d in (Get-Disk | Sort-Object Number)){
  $parts = foreach($p in (Get-Partition -DiskNumber $d.Number)){
    $v = $p | Get-Volume
    $l = [string]$p.DriveLetter
    if ($l -eq [string][char]0) { $l = '' }
    [pscustomobject]@{Number=[int]$p.PartitionNumber;Offset=[uint64]$p.Offset;Size=[uint64]$p.Size;Letter=$l;Type=[string]$p.Type;GptType=[string]$p.GptType;MbrType=$p.MbrType;IsActive=[bool]$p.IsActive;IsBoot=[bool]$p.IsBoot;IsSystem=[bool]$p.IsSystem;IsHidden=[bool]$p.IsHidden;FileSystem=[string]$v.FileSystem;Label=[string]$v.FileSystemLabel;Free=$v.SizeRemaining;VolSize=$v.Size}
  }
  [pscustomobject]@{Number=[int]$d.Number;Model=[string]$d.FriendlyName;Size=[uint64]$d.Size;Style=[string]$d.PartitionStyle;Sector=[int]$d.LogicalSectorSize;IsBoot=[bool]$d.IsBoot;IsSystem=[bool]$d.IsSystem;Bus=[string]$d.BusType;Offline=[bool]$d.IsOffline;Uid=[string]$d.UniqueId;Parts=@($parts)}
}
ConvertTo-Json -InputObject @($out) -Depth 5 -Compress
"#;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawPart {
    number: u32,
    offset: u64,
    size: u64,
    letter: Option<String>,
    #[serde(rename = "Type")]
    ptype: Option<String>,
    gpt_type: Option<String>,
    mbr_type: Option<u32>,
    is_active: bool,
    is_boot: bool,
    is_system: bool,
    is_hidden: bool,
    file_system: Option<String>,
    label: Option<String>,
    free: Option<u64>,
    vol_size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawDisk {
    number: u32,
    model: Option<String>,
    size: u64,
    style: Option<String>,
    sector: Option<u64>,
    is_boot: bool,
    is_system: bool,
    bus: Option<String>,
    offline: bool,
    uid: Option<String>,
    parts: Vec<RawPart>,
}

pub fn load_disks() -> Result<Vec<Disk>, String> {
    let mut text = String::new();
    crate::worker::run_ps_file(SCRIPT, &mut |l| {
        text.push_str(l);
        text.push('\n');
    })?;
    let start = text.find('[').ok_or_else(|| format!("Réponse inattendue : {}", text.trim()))?;
    let raw: Vec<RawDisk> = serde_json::from_str(text[start..].trim()).map_err(|e| e.to_string())?;
    if raw.is_empty() {
        return Err("Aucun disque n'a été trouvé (droits insuffisants ?)".into());
    }
    Ok(raw.into_iter().map(convert).collect())
}

fn convert(d: RawDisk) -> Disk {
    let style = match d.style.as_deref().unwrap_or("").to_ascii_uppercase().as_str() {
        "GPT" => Style::Gpt,
        "MBR" => Style::Mbr,
        _ => Style::Raw,
    };
    let sector = d.sector.filter(|&s| s >= 512).unwrap_or(512);
    let mut dynamic = false;
    let parts = d
        .parts
        .into_iter()
        .map(|p| {
            let gpt = p.gpt_type.unwrap_or_default().trim_matches(|c| c == '{' || c == '}').to_ascii_uppercase();
            let ptype = p.ptype.unwrap_or_default();
            let kind = if gpt == GUID_EFI || p.mbr_type == Some(0xEF) || ptype == "System" {
                Kind::Efi
            } else if gpt == GUID_MSR || ptype == "Reserved" {
                Kind::Msr
            } else if gpt == GUID_RECOVERY || p.mbr_type == Some(0x27) || ptype == "Recovery" {
                Kind::Recovery
            } else if gpt == GUID_LDM_META {
                Kind::LdmMeta
            } else if gpt == GUID_LDM_DATA || p.mbr_type == Some(0x42) {
                Kind::LdmData
            } else if matches!(p.mbr_type, Some(0x05) | Some(0x0F)) || ptype == "Extended" {
                Kind::Extended
            } else if ptype == "Logical" {
                Kind::Logical
            } else {
                Kind::Data
            };
            if matches!(kind, Kind::LdmMeta | Kind::LdmData) {
                dynamic = true;
            }
            let used = match (p.vol_size, p.free) {
                (Some(v), Some(f)) if v > 0 => Some(v.saturating_sub(f)),
                _ => None,
            };
            Partition {
                number: p.number,
                offset: p.offset,
                size: p.size,
                letter: p.letter.and_then(|l| l.chars().next()).filter(|c| c.is_ascii_alphabetic()),
                fs: Fs::parse(&p.file_system.unwrap_or_default()),
                label: p.label.unwrap_or_default(),
                used,
                kind,
                is_system: p.is_system,
                is_boot: p.is_boot,
                is_active: p.is_active,
                hidden: p.is_hidden,
                pending: false,
            }
        })
        .collect();
    Disk {
        number: d.number,
        model: d.model.unwrap_or_else(|| "Disque".into()),
        size: d.size,
        sector,
        style,
        dynamic,
        is_system: d.is_system,
        is_boot: d.is_boot,
        bus: d.bus.unwrap_or_default(),
        offline: d.offline,
        uid: d.uid.unwrap_or_default(),
        parts,
        pending: false,
    }
}

/// Example disks shown when the real ones can't be read (or with --demo).
pub fn demo_disks() -> Vec<Disk> {
    let p = |number, offset: u64, size: u64, letter: Option<char>, fs: Fs, label: &str, used: Option<u64>, kind| Partition {
        number,
        offset,
        size,
        letter,
        fs,
        label: label.into(),
        used,
        kind,
        is_system: kind == Kind::Efi,
        is_boot: letter == Some('C'),
        is_active: false,
        hidden: false,
        pending: false,
    };
    vec![
        Disk {
            number: 0,
            model: "SSD NVMe exemple".into(),
            size: 512 * GIB,
            sector: 512,
            style: Style::Gpt,
            dynamic: false,
            is_system: true,
            is_boot: true,
            bus: "NVMe".into(),
            offline: false,
            uid: "demo-0".into(),
            parts: vec![
                p(1, MIB, 100 * MIB, None, Fs::Fat32, "", Some(32 * MIB), Kind::Efi),
                p(2, 101 * MIB, 16 * MIB, None, Fs::Raw, "", None, Kind::Msr),
                p(3, 117 * MIB, 380 * GIB, Some('C'), Fs::Ntfs, "Windows", Some(212 * GIB), Kind::Data),
                p(4, 117 * MIB + 380 * GIB, 750 * MIB, None, Fs::Ntfs, "", Some(560 * MIB), Kind::Recovery),
            ],
            pending: false,
        },
        Disk {
            number: 1,
            model: "Disque dur exemple".into(),
            size: 1000 * GIB,
            sector: 512,
            style: Style::Mbr,
            dynamic: false,
            is_system: false,
            is_boot: false,
            bus: "SATA".into(),
            offline: false,
            uid: "demo-1".into(),
            parts: vec![
                p(1, MIB, 300 * GIB, Some('D'), Fs::Ntfs, "Données", Some(120 * GIB), Kind::Data),
                p(2, MIB + 300 * GIB, 200 * GIB, Some('E'), Fs::Ntfs, "Photos", Some(150 * GIB), Kind::Data),
                p(3, MIB + 600 * GIB, 64 * GIB, Some('F'), Fs::ExFat, "Échange", Some(8 * GIB), Kind::Data),
            ],
            pending: false,
        },
        Disk {
            number: 2,
            model: "Clé USB exemple".into(),
            size: 32 * GIB,
            sector: 512,
            style: Style::Mbr,
            dynamic: false,
            is_system: false,
            is_boot: false,
            bus: "USB".into(),
            offline: false,
            uid: "demo-2".into(),
            parts: vec![p(1, MIB, 32 * GIB - 2 * MIB, Some('G'), Fs::Fat32, "CLE", Some(3 * GIB), Kind::Data)],
            pending: false,
        },
    ]
}
