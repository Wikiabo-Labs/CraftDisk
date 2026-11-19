//! Bootable WinPE media with DiskCraft inside, built with Microsoft's free ADK tools.

use crate::ops::ps_quote;
use crate::plan::Step;
use std::path::PathBuf;

pub fn adk_root() -> PathBuf {
    let pf = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    PathBuf::from(pf).join(r"Windows Kits\10\Assessment and Deployment Kit")
}

/// Whether the ADK and its WinPE add-on are installed.
pub fn adk_ready() -> (bool, bool) {
    let r = adk_root();
    (
        r.join(r"Deployment Tools\DandISetEnv.bat").exists(),
        r.join(r"Windows Preinstallation Environment\copype.cmd").exists(),
    )
}

pub fn install_steps() -> Vec<Step> {
    vec![Step::Ps {
        script: "# Installer les outils WinPE gratuits de Microsoft\nwinget install -e --id Microsoft.WindowsADK --silent --accept-package-agreements --accept-source-agreements\nwinget install -e --id Microsoft.ADKPEAddon --silent --accept-package-agreements --accept-source-agreements\nWrite-Output 'Outils installés.'".into(),
    }]
}

pub enum Target {
    Usb(char),
    Iso(String),
}

pub fn build_steps(target: &Target) -> Vec<Step> {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let adk = adk_root().display().to_string();
    let work = r"C:\DiskCraftPE";
    let media = match target {
        Target::Usb(l) => format!("/UFD {work} {l}:"),
        Target::Iso(p) => format!("/ISO {work} {}", ps_quote(p)),
    };
    let script = format!(
        r#"# Créer le support de démarrage WinPE
$adk = {adk}
$work = '{work}'
$env = "`"$adk\Deployment Tools\DandISetEnv.bat`""
if (Test-Path $work) {{ dism /Unmount-Image /MountDir:"$work\mount" /Discard 2>$null | Out-Null; Remove-Item $work -Recurse -Force }}
Write-Output 'Copie des fichiers WinPE...'
cmd /c "call $env && copype amd64 $work"
if ($LASTEXITCODE -ne 0) {{ throw 'copype a échoué.' }}
dism /Mount-Image /ImageFile:"$work\media\sources\boot.wim" /Index:1 /MountDir:"$work\mount"
if ($LASTEXITCODE -ne 0) {{ throw 'Montage de l image impossible.' }}
$oc = "$adk\Windows Preinstallation Environment\amd64\WinPE_OCs"
foreach ($pkg in 'WinPE-WMI','WinPE-NetFx','WinPE-Scripting','WinPE-PowerShell','WinPE-StorageWMI','WinPE-DismCmdlets') {{
  Write-Output "Ajout de $pkg"
  dism /Image:"$work\mount" /Add-Package /PackagePath:"$oc\$pkg.cab" | Out-Null
  if (Test-Path "$oc\en-us\${{pkg}}_en-us.cab") {{ dism /Image:"$work\mount" /Add-Package /PackagePath:"$oc\en-us\${{pkg}}_en-us.cab" | Out-Null }}
}}
New-Item -ItemType Directory -Force "$work\mount\DiskCraft" | Out-Null
Copy-Item {exe} "$work\mount\DiskCraft\DiskCraft.exe" -Force
Add-Content "$work\mount\Windows\System32\startnet.cmd" "`r`nstart X:\DiskCraft\DiskCraft.exe"
dism /Unmount-Image /MountDir:"$work\mount" /Commit
if ($LASTEXITCODE -ne 0) {{ throw 'Enregistrement de l image impossible.' }}
Write-Output 'Écriture du support...'
cmd /c "call $env && echo Y | MakeWinPEMedia {media}"
if ($LASTEXITCODE -ne 0) {{ throw 'MakeWinPEMedia a échoué.' }}
Write-Output 'Support de démarrage prêt.'"#,
        adk = ps_quote(&adk),
        exe = ps_quote(&exe),
    );
    vec![Step::Ps { script }]
}
