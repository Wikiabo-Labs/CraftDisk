//! English / French. The language follows Windows (French Windows → French, otherwise
//! English) until the person picks one; the choice is remembered.
//! Every text in the code carries both versions: `tr!("français", "English")` and
//! `trf!("format français {x}", "English format {x}", ...)`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(true);

/// Plain text in both languages.
#[macro_export]
macro_rules! tr {
    ($fr:expr, $en:expr $(,)?) => {
        if $crate::i18n::en() {
            $en
        } else {
            $fr
        }
    };
}

/// `format!` in both languages (same arguments).
#[macro_export]
macro_rules! trf {
    ($fr:literal, $en:literal $($rest:tt)*) => {
        if $crate::i18n::en() {
            format!($en $($rest)*)
        } else {
            format!($fr $($rest)*)
        }
    };
}

pub fn en() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

fn settings_file() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("DiskCraft").join("language.txt"))
}

#[cfg(windows)]
fn windows_is_french() -> bool {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // Primary language id 0x0C is French (France, Belgium, Canada, Switzerland...).
    (unsafe { GetUserDefaultUILanguage() } & 0x3FF) == 0x0C
}
#[cfg(not(windows))]
fn windows_is_french() -> bool {
    std::env::var("LANG").map(|l| l.starts_with("fr")).unwrap_or(false)
}

/// Picks the language: DISKCRAFT_LANG (fr/en), then the saved choice, then Windows' language.
pub fn init() {
    let pick = std::env::var("DISKCRAFT_LANG")
        .ok()
        .or_else(|| settings_file().and_then(|p| std::fs::read_to_string(p).ok()))
        .map(|s| s.trim().to_ascii_lowercase());
    let english = match pick.as_deref() {
        Some("fr") => false,
        Some("en") => true,
        _ => !windows_is_french(),
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

/// Switches the language and remembers it.
pub fn set_english(english: bool) {
    ENGLISH.store(english, Ordering::Relaxed);
    if let Some(p) = settings_file() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, if english { "en" } else { "fr" });
    }
}

/// Messages the PowerShell scripts print (in French) shown in English in the log window.
pub fn log_line(fr: &str) -> String {
    if !en() {
        return fr.to_string();
    }
    const PHRASES: &[(&str, &str)] = &[
        ("diskpart a échoué : vos fichiers sont dans", "diskpart failed: your files are in"),
        ("La remise en place de", "Putting back"),
        (": a échoué : les fichiers restent dans", ": failed: the files are still in"),
        ("La copie de ", "Copying "),
        (": a échoué.", ": failed."),
        ("Partition introuvable (disque", "Partition not found (disk"),
        (", position", ", position"),
        ("Aucun volume avec une lettre n a été trouvé sur ce disque.", "No volume with a drive letter was found on this disk."),
        ("bcdboot a échoué.", "bcdboot failed."),
        ("copype a échoué.", "copype failed."),
        ("Enregistrement de l image impossible.", "Could not save the image."),
        ("La copie du contenu a échoué.", "Copying the contents failed."),
        ("La copie temporaire a échoué.", "The temporary copy failed."),
        ("Le disque ne remplit pas les conditions de mbr2gpt.", "The disk does not meet mbr2gpt's conditions."),
        ("MakeWinPEMedia a échoué.", "MakeWinPEMedia failed."),
        ("mbr2gpt a échoué.", "mbr2gpt failed."),
        ("Montage de l image impossible.", "Could not mount the image."),
        ("Volume introuvable", "Volume not found"),
        ("Ajout de ", "Adding "),
        ("Copie de ", "Copying "),
        (" vers ", " to "),
        ("bootsect introuvable : le code de démarrage copié du disque source est conservé.", "bootsect not found: the boot code copied from the source disk is kept."),
        ("Copie des fichiers WinPE...", "Copying the WinPE files..."),
        ("Démarrage réparé : choisissez le nouveau disque dans le menu de démarrage du PC.", "Boot repaired: choose the new disk in the PC's boot menu."),
        ("Écriture du support...", "Writing the media..."),
        ("Outils installés.", "Tools installed."),
        ("Pensez à passer le BIOS du PC en mode UEFI avant de redémarrer.", "Remember to switch the PC's firmware to UEFI mode before restarting."),
        ("Support de démarrage prêt.", "Boot media ready."),
    ];
    let mut s = fr.to_string();
    for (f, e) in PHRASES {
        s = s.replace(f, e);
    }
    s
}

/// Titles of the PowerShell steps (written in French inside the scripts) shown in English.
pub fn step_title(fr: &str) -> String {
    if !en() {
        return fr.to_string();
    }
    const PHRASES: &[(&str, &str)] = &[
        ("Agrandir la dernière partition au disque cible", "Extend the last partition to the target disk"),
        ("Changer la lettre de lecteur", "Change the drive letter"),
        ("Changer le nom de la partition", "Change the partition label"),
        ("Convertir le disque de Windows en GPT", "Convert the Windows disk to GPT"),
        ("Convertir le disque dynamique", "Convert dynamic disk"),
        ("en disque de base", "to a basic disk"),
        ("Créer le support de démarrage WinPE", "Create the WinPE boot media"),
        ("Créer une partition", "Create a partition"),
        ("Donner une lettre à la copie", "Give the copy a drive letter"),
        ("Formater en", "Format as"),
        ("Fusionner deux partitions", "Merge two partitions"),
        ("Initialiser le disque", "Initialize disk"),
        ("Installer les outils WinPE gratuits de Microsoft", "Install Microsoft's free WinPE tools"),
        ("Mettre en pause l'environnement de récupération", "Pause the recovery environment"),
        ("Préparer la place de la copie", "Prepare room for the copy"),
        ("Préparer le disque cible", "Prepare target disk"),
        ("Préparer le disque", "Prepare disk"),
        ("Rechercher les erreurs", "Look for errors"),
        ("Redimensionner la partition", "Resize the partition"),
        ("Relire le disque", "Read again disk"),
        ("Remettre le disque", "Bring disk"),
        ("en ligne", "back online"),
        ("Remettre les fichiers sur", "Put the files back on"),
        ("Remonter la partition déplacée", "Mount the moved partition again"),
        ("Rendre la partition active", "Set the partition active"),
        ("Réparation du démarrage", "Boot repair"),
        ("Supprimer la partition", "Delete the partition"),
        ("Vérifier et copier les fichiers de", "Check and copy the files of"),
        ("Vérifier et réparer les erreurs", "Check and repair errors"),
        (" en GPT", " as GPT"),
        (" en MBR", " as MBR"),
        ("Commande", "Command"),
    ];
    let mut s = fr.to_string();
    for (f, e) in PHRASES {
        s = s.replace(f, e);
    }
    s
}
