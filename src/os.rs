//! Small OS integrations shared by the windows: reveal in the file manager, put files or
//! images on the clipboard, and the user's media folders. All through the OS's own tools.

use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn run(cmd: &mut Command, input: Option<&str>) -> Result<()> {
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().with_context(|| format!("running {:?}", cmd.get_program()))?;
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(text.as_bytes())?;
    }
    if !child.wait()?.success() {
        bail!("{:?} failed", cmd.get_program());
    }
    Ok(())
}

/// AppleScript / PowerShell string literals.
fn applescript_str(p: &Path) -> String {
    format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\""))
}
fn powershell_str(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "''"))
}

pub fn reveal(path: &Path) -> Result<()> {
    match std::env::consts::OS {
        "macos" => run(Command::new("open").arg("-R").arg(path), None),
        // explorer exits 1 even when it worked.
        "windows" => Command::new("explorer").arg(format!("/select,{}", path.display())).status().map(|_| ()).context("running explorer"),
        _ => run(Command::new("xdg-open").arg(path.parent().unwrap_or(path)), None),
    }
}

/// Puts the file itself on the clipboard, so pasting into Finder, chat apps or mail attaches it.
pub fn copy_file(path: &Path) -> Result<()> {
    match std::env::consts::OS {
        "macos" => run(Command::new("osascript").args(["-e", &format!("set the clipboard to POSIX file {}", applescript_str(path))]), None),
        "windows" => run(Command::new("powershell").args(["-NoProfile", "-Command", &format!("Set-Clipboard -Path {}", powershell_str(path))]), None),
        _ => linux_clip("text/uri-list", None, Some(&format!("file://{}\n", path.display()))),
    }
}

/// Puts a PNG file's image on the clipboard.
pub fn copy_png(path: &Path) -> Result<()> {
    match std::env::consts::OS {
        "macos" => run(Command::new("osascript").args(["-e", &format!("set the clipboard to (read (POSIX file {}) as «class PNGf»)", applescript_str(path))]), None),
        "windows" => run(
            Command::new("powershell").args([
                "-NoProfile",
                "-Command",
                &format!("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; [System.Windows.Forms.Clipboard]::SetImage([System.Drawing.Image]::FromFile({}))", powershell_str(path)),
            ]),
            None,
        ),
        _ => linux_clip("image/png", Some(path), None),
    }
}

/// wl-copy on Wayland, xclip on X11; one of them must be installed.
fn linux_clip(mime: &str, file: Option<&Path>, text: Option<&str>) -> Result<()> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let mut cmd = if wayland { Command::new("wl-copy") } else { Command::new("xclip") };
    if wayland {
        cmd.args(["-t", mime]);
    } else {
        cmd.args(["-selection", "clipboard", "-t", mime]);
    }
    match file {
        Some(f) => run(cmd.stdin(std::fs::File::open(f)?), None),
        None => run(&mut cmd, text),
    }
    .context("copying needs wl-copy (Wayland) or xclip (X11) installed")
}

/// The user's Movies/Videos or Pictures folder, else their home folder.
pub fn media_dir(pictures: bool) -> PathBuf {
    let home = PathBuf::from(std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into()));
    let names: &[&str] = match (pictures, std::env::consts::OS) {
        (true, _) => &["Pictures", "Downloads"],
        (false, "macos") => &["Movies", "Downloads"],
        (false, _) => &["Videos", "Downloads"],
    };
    names.iter().map(|n| home.join(n)).find(|d| d.is_dir()).unwrap_or(home)
}

/// `dir/name.ext`, or `dir/name (n).ext` when taken.
pub fn unique(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let name = name.replace(['/', '\\', ':'], "-");
    (0..).map(|n| dir.join(if n == 0 { format!("{name}.{ext}") } else { format!("{name} ({n}).{ext}") })).find(|p| !p.exists()).expect("some name is free")
}
