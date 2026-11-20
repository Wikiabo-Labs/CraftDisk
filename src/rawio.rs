//! Raw access to disks and volumes (Windows).

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(windows)]
use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};

const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;
const FSCTL_ALLOW_EXTENDED_DASD_IO: u32 = 0x0009_0083;
const IOCTL_DISK_GET_LENGTH_INFO: u32 = 0x0007_405C;
const IOCTL_DISK_UPDATE_PROPERTIES: u32 = 0x0007_0140;

pub struct Dev {
    pub file: File,
    pub path: String,
}

#[cfg(windows)]
fn ioctl(f: &File, code: u32, out: &mut [u8]) -> std::io::Result<u32> {
    use windows_sys::Win32::System::IO::DeviceIoControl;
    let mut ret: u32 = 0;
    let ok = unsafe {
        DeviceIoControl(
            f.as_raw_handle() as _,
            code,
            std::ptr::null(),
            0,
            if out.is_empty() { std::ptr::null_mut() } else { out.as_mut_ptr() as _ },
            out.len() as u32,
            &mut ret,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}
#[cfg(not(windows))]
fn ioctl(_f: &File, _code: u32, _out: &mut [u8]) -> std::io::Result<u32> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, crate::tr!("Windows seulement", "Windows only")))
}

impl Dev {
    pub fn open(path: &str, write: bool) -> Result<Dev, String> {
        let mut o = OpenOptions::new();
        o.read(true).write(write);
        #[cfg(windows)]
        o.share_mode(3); // FILE_SHARE_READ | FILE_SHARE_WRITE
        let file = o.open(path).map_err(|e| crate::trf!("Ouverture de {path} impossible : {e}", "Cannot open {path}: {e}"))?;
        Ok(Dev { file, path: path.to_string() })
    }
    pub fn disk(n: u32, write: bool) -> Result<Dev, String> {
        Dev::open(&format!(r"\\.\PhysicalDrive{n}"), write)
    }
    pub fn len(&self) -> Result<u64, String> {
        let mut b = [0u8; 8];
        ioctl(&self.file, IOCTL_DISK_GET_LENGTH_INFO, &mut b).map_err(|e| crate::trf!("Taille de {} inconnue : {e}", "Size of {} unknown: {e}", self.path))?;
        Ok(u64::from_le_bytes(b))
    }
    /// Take the volume away from Windows so it can be written raw.
    pub fn lock_dismount(&self) -> Result<(), String> {
        let _ = ioctl(&self.file, FSCTL_ALLOW_EXTENDED_DASD_IO, &mut []);
        let mut tries = 0;
        while ioctl(&self.file, FSCTL_LOCK_VOLUME, &mut []).is_err() {
            tries += 1;
            if tries > 10 {
                return Err(crate::tr!("Le volume est utilisé par un programme : fermez-le et réessayez.", "The volume is being used by a program: close it and try again.").into());
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        ioctl(&self.file, FSCTL_DISMOUNT_VOLUME, &mut []).map_err(|e| crate::trf!("Démontage impossible : {e}", "Cannot dismount: {e}"))?;
        Ok(())
    }
    pub fn update_properties(&self) {
        let _ = ioctl(&self.file, IOCTL_DISK_UPDATE_PROPERTIES, &mut []);
    }
    pub fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> Result<(), String> {
        self.file.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        self.file.read_exact(buf).map_err(|e| crate::trf!("Lecture à {pos} sur {} : {e}", "Read at {pos} on {}: {e}", self.path))
    }
    pub fn write_at(&mut self, pos: u64, buf: &[u8]) -> Result<(), String> {
        self.file.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        self.file.write_all(buf).map_err(|e| crate::trf!("Écriture à {pos} sur {} : {e}", "Write at {pos} on {}: {e}", self.path))
    }
}

/// Whether this process runs with administrator rights (elevated token).
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: plain Win32 calls with valid out-pointers; the token handle is closed below.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(token, TokenElevation, &mut elevation as *mut _ as *mut _, std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut len);
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}
#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Simple fast pseudo-random filler for wiping.
pub struct XorShift(pub u64);
impl XorShift {
    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            let b = x.to_le_bytes();
            chunk.copy_from_slice(&b[..chunk.len()]);
        }
    }
}

pub fn seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    t ^ 0x9E37_79B9_7F4A_7C15 | 1
}
