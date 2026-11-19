//! Minimal stand-in for the `webbrowser` crate: opens a link with the system's default handler.

pub fn open(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd").args(["/C", "start", "", url]).creation_flags(0x0800_0000).spawn().map(|_| ())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
}
