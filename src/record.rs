//! Studio recording: one ffmpeg process per source (screen, camera, mic) plus a
//! cursor-polling thread. Every pause/resume starts a new segment folder, like Cap.
//!
//! Sync: every ffmpeg input is stamped with wall-clock time shifted by the
//! segment's start (`-use_wallclock_as_timestamps` + `-itsoffset -T0` + `-copyts`),
//! so each file's first timestamp *is* its offset from the segment start.
//! We read it back with ffprobe into `start_time`, which is how Cap stores it too.

use crate::project::{self, Click, CursorLog, Meta, Move, Segment, Track};
use anyhow::{Context, Result, bail};
use device_query::{DeviceQuery, DeviceState};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OS: &str = std::env::consts::OS;

pub struct Sources {
    /// Index into `screens()`.
    pub display: usize,
    /// Record only this part of the display (in the display's own coordinates).
    pub area: Option<Area>,
    /// Record only this window (instead of the display).
    pub window: Option<Window>,
    pub camera: Option<String>,
    pub mic: Option<String>,
    /// Also record what the computer plays.
    pub system_audio: bool,
    pub fps: u32,
    /// Record just the camera, saved as the display track (Cap stores its camera-only recordings this way).
    pub camera_only: bool,
    /// Bake the OS cursor into the video (Instant mode) instead of recording its moves for the editor.
    pub show_cursor: bool,
    /// Other windows to leave out of display recordings (macOS only).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub exclude: Vec<u32>,
    /// "balanced", "compatibility" (smaller files) or "ultra".
    pub quality: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Serialize)]
pub struct Screen {
    /// OS display id (CGDirectDisplayID on macOS).
    pub id: u32,
    pub name: String,
    /// Position and size in the OS's cursor coordinates (points on macOS, pixels elsewhere).
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Physical pixels per coordinate unit.
    pub scale: f64,
}

/// Another app's window, in the same coordinates as `Screen`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub id: u32,
    pub title: String,
    pub app: String,
    /// The app's bundle id on macOS ("" elsewhere), for excluded-window rules.
    #[serde(default)]
    pub bundle: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Index into `screens()` of the screen holding the window's centre.
    pub display: usize,
}

#[derive(Clone, Serialize)]
pub struct Device {
    /// What ffmpeg needs to open it.
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct Devices {
    pub screens: Vec<Screen>,
    pub cameras: Vec<Device>,
    pub mics: Vec<Device>,
}

pub fn screens() -> Vec<Screen> {
    let mut all = display_info::DisplayInfo::all().unwrap_or_default();
    all.sort_by_key(|d| !d.is_primary);
    let scale = |d: &display_info::DisplayInfo| if OS == "macos" { d.scale_factor as f64 } else { 1.0 };
    all.iter()
        .map(|d| Screen { id: d.id, name: d.friendly_name.clone(), x: d.x as f64, y: d.y as f64, w: d.width as f64, h: d.height as f64, scale: scale(d) })
        .collect()
}

/// Other apps' windows, topmost first.
pub fn windows() -> Vec<Window> {
    #[cfg(target_os = "macos")]
    let mut all = crate::sck::windows().unwrap_or_default();
    #[cfg(not(target_os = "macos"))]
    let mut all = crate::oswin::windows();
    // Browsers and some apps put untitled strips (toolbars, popups) on screen as separate windows.
    let titled: std::collections::HashSet<String> = all.iter().filter(|w| !w.title.is_empty()).map(|w| w.app.clone()).collect();
    all.retain(|w| !w.title.is_empty() || !titled.contains(&w.app));
    let screens = screens();
    for w in &mut all {
        let (cx, cy) = (w.x + w.w / 2.0, w.y + w.h / 2.0);
        w.display = screens.iter().position(|s| cx >= s.x && cx < s.x + s.w && cy >= s.y && cy < s.y + s.h).unwrap_or(0);
    }
    all
}

/// Default device when `--camera`/`--mic` is passed without a name. Windows
/// dshow has no "default" device, so the user must pick one from `devices`.
pub fn default_device(camera: bool) -> Result<String> {
    Ok(match (OS, camera) {
        ("macos", _) => "0".into(),
        ("windows", _) => bail!("on Windows pass a device name; run `precap devices` to list them"),
        (_, true) => "/dev/video0".into(),
        (_, false) => "default".into(),
    })
}

fn ffmpeg_text(args: &[&str]) -> Result<String> {
    // ffmpeg exits non-zero after listing devices; the list itself is what we want.
    let out = Command::new("ffmpeg").arg("-hide_banner").args(args).output().context("running ffmpeg")?;
    Ok(String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout))
}

/// ffmpeg's log output, killing it after `limit`.
fn ffmpeg_text_within(args: &[&str], limit: Duration) -> String {
    let Ok(mut child) = Command::new("ffmpeg").arg("-hide_banner").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn() else {
        return String::new();
    };
    track(&child, true);
    let mut stderr = child.stderr.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + limit;
    while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    track(&child, false);
    reader.join().unwrap_or_default()
}

pub fn devices() -> Result<Devices> {
    let (cameras, mics) = match OS {
        "macos" => parse_avfoundation(&ffmpeg_text(&["-f", "avfoundation", "-list_devices", "true", "-i", ""])?),
        "windows" => parse_dshow(&ffmpeg_text(&["-f", "dshow", "-list_devices", "true", "-i", "dummy"])?),
        _ => (parse_sources(&ffmpeg_text(&["-sources", "v4l2"])?), parse_sources(&ffmpeg_text(&["-sources", "pulse"])?).into_iter().filter(|d| !d.id.ends_with(".monitor")).collect()),
    };
    Ok(Devices { screens: screens(), cameras, mics })
}

/// `[AVFoundation indev @ 0x..] [0] FaceTime HD Camera` lines, video list then audio list.
/// Devices are opened by name, since indexes shift when devices come and go; a name with
/// ':' would break avfoundation's "video:audio" syntax, so those fall back to the index.
/// Screens ("Capture screen N") are not cameras.
fn parse_avfoundation(text: &str) -> (Vec<Device>, Vec<Device>) {
    let (video, audio) = text.split_once("audio devices:").unwrap_or((text, ""));
    let parse = |part: &str| -> Vec<Device> {
        part.lines()
            .filter_map(|l| {
                let rest = l.split("] [").nth(1)?;
                let (id, name) = rest.split_once("] ")?;
                id.parse::<u32>().ok()?;
                let name = name.trim();
                let id = if name.contains(':') { id } else { name };
                Some(Device { id: id.into(), name: name.into() })
            })
            .collect()
    };
    (parse(video).into_iter().filter(|d| !d.name.starts_with("Capture screen")).collect(), parse(audio))
}

/// `[dshow @ 000..] "Integrated Camera" (video)`; dshow opens devices by name.
fn parse_dshow(text: &str) -> (Vec<Device>, Vec<Device>) {
    let mut out = (Vec::new(), Vec::new());
    for l in text.lines() {
        let Some((name, kind)) = l.split_once('"').and_then(|(_, r)| r.split_once('"')) else { continue };
        let d = Device { id: name.into(), name: name.into() };
        match kind.trim() {
            "(video)" => out.0.push(d),
            "(audio)" => out.1.push(d),
            _ => {}
        }
    }
    out
}

/// `ffmpeg -sources` lines: `* /dev/video0 [Integrated Camera]`.
fn parse_sources(text: &str) -> Vec<Device> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim().trim_start_matches('*').trim();
            let (id, name) = l.split_once(" [")?;
            Some(Device { id: id.into(), name: name.trim_end_matches(']').into() })
        })
        .collect()
}

/// Prints the platform's capture devices.
pub fn list_devices() -> Result<()> {
    let d = devices()?;
    for (i, s) in d.screens.iter().enumerate() {
        println!("screen {i}: {} ({}x{})", s.name, s.w, s.h);
    }
    for c in &d.cameras {
        println!("camera {:?}: {}", c.id, c.name);
    }
    for m in &d.mics {
        println!("mic    {:?}: {}", m.id, m.name);
    }
    for w in windows() {
        println!("window {}: {} — {} ({}x{} at {},{} on screen {})", w.id, w.app, w.title, w.w, w.h, w.x, w.y, w.display);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Kind {
    Screen,
    Camera,
    Mic,
}

/// ffmpeg input arguments per platform. The OS cursor is only drawn in when asked
/// (Instant); otherwise we record its position and draw our own, smoothed one at export.
fn screen_input(display: usize, screen: Option<&Screen>, window: Option<&Window>, fps: u32, cursor: bool) -> Vec<String> {
    let (fps, cursor) = (fps.to_string(), if cursor { "1" } else { "0" });
    let mut a: Vec<String> = Vec::new();
    let mut push = |v: &[&str]| a.extend(v.iter().map(|s| s.to_string()));
    match (OS, screen, window) {
        // ScreenCaptureKit (see sck.rs) writes raw frames to ffmpeg's stdin.
        ("macos", s, w) => {
            let (w, h) = capture_size(s, w);
            push(&["-f", "rawvideo", "-pix_fmt", "nv12", "-s", &format!("{w}x{h}"), "-i", "pipe:0"]);
        }
        ("windows", _, Some(w)) => push(&["-f", "gdigrab", "-draw_mouse", cursor, "-framerate", &fps, "-i", &format!("title={}", w.title)]),
        ("windows", Some(s), _) if display > 0 => push(&[
            "-f", "gdigrab", "-draw_mouse", cursor, "-framerate", &fps, "-offset_x", &s.x.to_string(), "-offset_y", &s.y.to_string(),
            "-video_size", &format!("{}x{}", s.w, s.h), "-i", "desktop",
        ]),
        ("windows", _, _) => push(&["-f", "gdigrab", "-draw_mouse", cursor, "-framerate", &fps, "-i", "desktop"]),
        // shortcut: X11 only (x11grab); Wayland needs a PipeWire portal capture backend.
        (_, s, w) => {
            let x = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
            push(&["-f", "x11grab", "-draw_mouse", cursor, "-framerate", &fps]);
            match (w, s) {
                (Some(w), _) => push(&["-window_id", &format!("0x{:x}", w.id), "-i", &x]),
                (None, Some(s)) => push(&["-video_size", &format!("{}x{}", s.w, s.h), "-i", &format!("{x}+{},{}", s.x, s.y)]),
                (None, None) => push(&["-i", &x]),
            }
        }
    }
    a
}

/// Pixel size ScreenCaptureKit delivers for a display or window, even for yuv420p.
fn capture_size(screen: Option<&Screen>, window: Option<&Window>) -> (usize, usize) {
    let scale = screen.map_or(1.0, |s| s.scale);
    match (window, screen) {
        (Some(w), _) => (((w.w * scale) as usize) & !1, ((w.h * scale) as usize) & !1),
        (None, Some(s)) => sck_size(s),
        (None, None) => (1920, 1080),
    }
}

/// Screen size in physical pixels, even for yuv420p.
fn sck_size(s: &Screen) -> (usize, usize) {
    (((s.w * s.scale) as usize) & !1, ((s.h * s.scale) as usize) & !1)
}

/// Some cameras reject 30 fps (OBS Virtual Camera only does 60); ffmpeg then lists the
/// modes they support and we take the one closest to 30. Probed once per device.
fn camera_mode(dev: &str) -> Vec<String> {
    static CACHE: Mutex<std::collections::BTreeMap<String, Vec<String>>> = Mutex::new(std::collections::BTreeMap::new());
    if let Some(m) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(dev) {
        return m.clone();
    }
    // A device that opens fine but sends nothing would block forever, hence the deadline.
    let text = ffmpeg_text_within(&["-f", "avfoundation", "-framerate", "30", "-i", dev, "-frames:v", "1", "-f", "null", "-"], Duration::from_secs(3));
    let mode = match parse_modes(&text) {
        Some((size, fps)) => vec!["-framerate".into(), fps, "-video_size".into(), size],
        None => vec!["-framerate".into(), "30".into()],
    };
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).insert(dev.into(), mode.clone());
    mode
}

/// `  1920x1080@[60.000000 60.000000]fps` lines after "is not supported by the device".
fn parse_modes(text: &str) -> Option<(String, String)> {
    if !text.contains("not supported") {
        return None;
    }
    text.lines()
        .filter_map(|l| {
            let (size, fps) = l.split_whitespace().find(|t| t.contains("@["))?.split_once("@[")?;
            let fps: f64 = fps.parse().ok()?;
            Some((size.to_string(), fps))
        })
        .min_by(|a, b| (a.1 - 30.0).abs().total_cmp(&(b.1 - 30.0).abs()))
        .map(|(size, fps)| (size, format!("{fps}")))
}

fn device_input(kind: Kind, dev: &str) -> Vec<String> {
    if let ("macos", Kind::Camera) = (OS, kind) {
        return [vec!["-f".into(), "avfoundation".into()], camera_mode(dev), vec!["-i".into(), dev.into()]].concat();
    }
    let a: &[&str] = match (OS, kind) {
        ("macos", _) => &["-f", "avfoundation", "-i", &format!(":{dev}")],
        ("windows", Kind::Camera) => &["-f", "dshow", "-i", &format!("video={dev}")],
        ("windows", _) => &["-f", "dshow", "-i", &format!("audio={dev}")],
        (_, Kind::Camera) => &["-f", "v4l2", "-framerate", "30", "-i", dev],
        _ => &["-f", "pulse", "-i", dev],
    };
    a.iter().map(|s| s.to_string()).collect()
}

/// What the computer plays. macOS: ScreenCaptureKit audio piped in (see `attach_system_audio`).
fn system_audio_input() -> Result<Vec<String>> {
    let a: Vec<&str> = match OS {
        "macos" => vec!["-f", "f32le", "-ar", "48000", "-ac", "2", "-i", "pipe:0"],
        "windows" => {
            // ffmpeg has no WASAPI loopback; a loopback recording device (Stereo Mix, VB-CABLE...) stands in.
            let (_, mics) = parse_dshow(&ffmpeg_text(&["-f", "dshow", "-list_devices", "true", "-i", "dummy"])?);
            let loopback = ["stereo mix", "what u hear", "virtual-audio-capturer", "cable output", "wave out"];
            let dev = mics
                .into_iter()
                .find(|d| loopback.iter().any(|l| d.name.to_lowercase().contains(l)))
                .context("system audio on Windows needs a loopback recording device: enable \"Stereo Mix\" in Sound settings > Recording, or install VB-CABLE")?;
            return Ok(vec!["-f".into(), "dshow".into(), "-i".into(), format!("audio={}", dev.id)]);
        }
        // PulseAudio and PipeWire both name the default output's monitor this way.
        _ => vec!["-f", "pulse", "-i", "@DEFAULT_MONITOR@"],
    };
    Ok(a.into_iter().map(String::from).collect())
}

/// Peak of a 16-bit sample block in dBFS, floored at -60 like Cap's meters.
fn level_db(peak: u16) -> f64 {
    (20.0 * (peak.max(1) as f64 / 32768.0).log10()).clamp(-60.0, 0.0)
}

/// Live mic level for the UI meter: its own small ffmpeg reading 16 kHz mono PCM,
/// reporting the peak of every 50 ms. Runs beside a recording (the OS shares the mic).
pub struct MicMeter(Child);

impl MicMeter {
    pub fn start(dev: &str, on_level: impl Fn(f64) + Send + 'static) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(device_input(Kind::Mic, dev))
            .args(["-ac", "1", "-ar", "16000", "-f", "s16le", "pipe:1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to start ffmpeg; is it installed and on PATH?")?;
        track(&child, true);
        let mut out = child.stdout.take().context("ffmpeg stdout")?;
        std::thread::spawn(move || {
            let mut block = [0u8; 1600];
            while out.read_exact(&mut block).is_ok() {
                let peak = block.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]).unsigned_abs()).max().unwrap_or(0);
                on_level(level_db(peak));
            }
        });
        Ok(Self(child))
    }
}

impl Drop for MicMeter {
    fn drop(&mut self) {
        track(&self.0, false);
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn codec_args(kind: Kind, quality: &str) -> Vec<&'static str> {
    // Cap's studio presets: Balanced, Smaller files, Maximum detail.
    let (bitrate, crf) = match quality {
        "compatibility" => ("6M", "23"),
        "ultra" => ("25M", "14"),
        _ => ("12M", "18"),
    };
    match kind {
        // Hardware H.264 on macOS (Cap uses VideoToolbox too); fast x264 elsewhere.
        Kind::Screen | Kind::Camera if OS == "macos" => vec!["-c:v", "h264_videotoolbox", "-b:v", bitrate, "-pix_fmt", "yuv420p"],
        Kind::Screen | Kind::Camera => vec!["-c:v", "libx264", "-preset", "ultrafast", "-crf", crf, "-pix_fmt", "yuv420p"],
        // async resampling turns jittery wall-clock audio stamps into a continuous stream.
        Kind::Mic => vec!["-af", "aresample=async=1000", "-c:a", "aac", "-b:a", "192k"],
    }
}

/// Latest camera JPEG, shared with the UI's camera bubble.
pub type CameraFrame = Arc<Mutex<Vec<u8>>>;

/// Square, small, ~20 fps MJPEG on stdout: cheap to show in a webview.
const PREVIEW_OUT: [&str; 10] = ["-vf", "crop='min(iw,ih)':'min(iw,ih)',scale=360:360", "-r", "20", "-q:v", "5", "-f", "mjpeg", "-an", "pipe:1"];

/// Splits ffmpeg's MJPEG stream into JPEGs and keeps the newest one.
fn pump_mjpeg(mut out: impl Read + Send + 'static, frame: CameraFrame) {
    std::thread::spawn(move || {
        let (mut buf, mut chunk) = (Vec::new(), [0u8; 65536]);
        while let Ok(n @ 1..) = out.read(&mut chunk) {
            buf.extend_from_slice(&chunk[..n]);
            // JPEG end marker; it can't occur inside the entropy-coded data (0xFF is byte-stuffed there).
            while let Some(end) = buf.windows(2).position(|w| w == [0xFF, 0xD9]) {
                let jpeg: Vec<u8> = buf.drain(..end + 2).collect();
                if let Some(start) = jpeg.windows(2).position(|w| w == [0xFF, 0xD8]) {
                    *frame.lock().unwrap_or_else(|e| e.into_inner()) = jpeg[start..].to_vec();
                }
            }
        }
    });
}

/// Every ffmpeg we started, so they can be stopped if the app quits or is killed
/// (otherwise they would keep capturing, camera light on).
static CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn track(child: &Child, add: bool) {
    let mut all = CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
    all.retain(|&p| p != child.id());
    if add {
        all.push(child.id());
    }
}

/// Stops every ffmpeg we started: EOF on piped input and SIGINT first (ffmpeg finalizes
/// its file), then SIGKILL for any still running after 3 s (a camera stuck waiting for
/// frames ignores SIGINT).
pub fn stop_all_children() {
    #[cfg(target_os = "macos")]
    crate::sck::close_all();
    let pids: Vec<String> = CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).drain(..).map(|p| p.to_string()).collect();
    if pids.is_empty() {
        return;
    }
    if OS == "windows" {
        for pid in &pids {
            let _ = Command::new("taskkill").args(["/PID", pid, "/F"]).status();
        }
        return;
    }
    let _ = Command::new("kill").arg("-INT").args(&pids).status();
    let alive = || pids.iter().any(|p| Command::new("kill").args(["-0", p]).stderr(Stdio::null()).status().is_ok_and(|s| s.success()));
    let deadline = Instant::now() + Duration::from_secs(3);
    while alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = Command::new("kill").arg("-KILL").args(&pids).stderr(Stdio::null()).status();
}

/// Camera preview while not recording (the recording takes over the device and keeps feeding the frame).
pub struct CameraPreview(Child);

impl CameraPreview {
    pub fn start(dev: &str, frame: CameraFrame) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(device_input(Kind::Camera, dev))
            .args(PREVIEW_OUT)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to start ffmpeg; is it installed and on PATH?")?;
        track(&child, true);
        pump_mjpeg(child.stdout.take().context("ffmpeg stdout")?, frame);
        Ok(Self(child))
    }
}

impl Drop for CameraPreview {
    fn drop(&mut self) {
        track(&self.0, false);
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Capture {
    child: Child,
    file: &'static str,
    /// Replaces the "q" on stdin when something else feeds ffmpeg (ScreenCaptureKit).
    stop_hook: Option<Box<dyn FnOnce() + Send>>,
}

fn spawn(dir: &Path, file: &'static str, kind: Kind, input: Vec<String>, crop: Option<String>, preview: Option<&CameraFrame>, t0: f64, quality: &str) -> Result<Capture> {
    let log = File::create(dir.join(format!("{file}.log")))?;
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-nostats", "-y", "-copyts", "-thread_queue_size", "1024"])
        .args(["-use_wallclock_as_timestamps", "1", "-itsoffset", &format!("-{t0:.6}")])
        .args(input);
    if let Some(crop) = crop {
        cmd.args(["-vf", &crop]);
    }
    // Matroska stays playable if we crash mid-recording (an mp4/m4a without its moov would not);
    // flushing every packet means a hard kill loses at most the last moment.
    cmd.args(codec_args(kind, quality)).args(["-flush_packets", "1"]).arg(dir.join(file));
    if preview.is_some() {
        cmd.args(PREVIEW_OUT);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(if preview.is_some() { Stdio::piped() } else { Stdio::null() })
        .stderr(log)
        .spawn()
        .context("failed to start ffmpeg; is it installed and on PATH?")?;
    track(&child, true);
    if let (Some(frame), Some(out)) = (preview, child.stdout.take()) {
        pump_mjpeg(out, frame.clone());
    }
    Ok(Capture { child, file, stop_hook: None })
}

fn stop(mut cap: Capture) -> Result<()> {
    match cap.stop_hook.take() {
        Some(hook) => hook(),
        // "q" on stdin makes ffmpeg flush and finalize the file.
        None => {
            if let Some(mut stdin) = cap.child.stdin.take() {
                let _ = stdin.write_all(b"q");
            }
        }
    }
    cap.child.wait()?;
    track(&cap.child, false);
    Ok(())
}

/// Feeds the screen ffmpeg from ScreenCaptureKit, which hides our own windows from the recording.
#[cfg(target_os = "macos")]
fn attach_screencapturekit(cap: &mut Capture, screen: Option<&Screen>, window: Option<&Window>, fps: u32, cursor: bool, exclude: &[u32]) -> Result<()> {
    use crate::sck::Source;
    let source = match (window, screen) {
        (Some(w), _) => Source::Window(w.id),
        (None, Some(s)) => Source::Display(s.id),
        (None, None) => bail!("no such display"),
    };
    let (w, h) = capture_size(screen, window);
    let stdin = cap.child.stdin.take().context("ffmpeg stdin")?;
    let capture = crate::sck::start(source, w, h, fps, cursor, exclude, stdin)?;
    cap.stop_hook = Some(Box::new(move || capture.stop()));
    Ok(())
}

#[cfg(target_os = "macos")]
fn attach_system_audio(cap: &mut Capture, screen: Option<&Screen>) -> Result<()> {
    let s = screen.context("no such display")?;
    let stdin = cap.child.stdin.take().context("ffmpeg stdin")?;
    let capture = crate::sck::start_audio(s.id, stdin)?;
    cap.stop_hook = Some(Box::new(move || capture.stop()));
    Ok(())
}

/// Saves a still of a display, an area of it, or a window to `out` (.png or .jpg),
/// leaving our own windows out, scaled down to at most `max_w` pixels wide.
pub fn screenshot(display: usize, area: Option<Area>, window: Option<&Window>, exclude: &[u32], out: &Path, max_w: Option<usize>) -> Result<()> {
    let screen = screens().into_iter().nth(window.map_or(display, |w| w.display));
    let scale = screen.as_ref().map_or(1.0, |s| s.scale);
    let (mut w, mut h) = match (area, window) {
        (_, Some(_)) => capture_size(screen.as_ref(), window),
        (Some(a), None) => ((a.w * scale) as usize, (a.h * scale) as usize),
        (None, None) => capture_size(screen.as_ref(), None),
    };
    if let Some(m) = max_w.filter(|&m| w > m) {
        (w, h) = (m, (h * m / w).max(1));
    }
    #[cfg(target_os = "macos")]
    {
        use crate::sck::Source;
        let source = match (window, &screen) {
            (Some(win), _) => Source::Window(win.id),
            (None, Some(s)) => Source::Display(s.id),
            (None, None) => bail!("no such display"),
        };
        let rect = area.filter(|_| window.is_none()).map(|a| (a.x, a.y, a.w, a.h));
        crate::sck::screenshot(source, rect, w.max(1), h.max(1), exclude, out)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = exclude; // X11 and GDI capture can't leave windows out.
        let mut filters = Vec::new();
        if let (Some(a), None) = (area, window) {
            filters.push(format!("crop={}:{}:{}:{}", (a.w * scale) as u32, (a.h * scale) as u32, (a.x * scale) as u32, (a.y * scale) as u32));
        }
        filters.push(format!("scale={w}:{h}"));
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(screen_input(display, screen.as_ref(), window, 30, false))
            .args(["-frames:v", "1", "-vf", &filters.join(",")])
            .arg(out)
            .status()
            .context("running ffmpeg")?;
        if !status.success() {
            bail!("ffmpeg could not take the screenshot");
        }
        Ok(())
    }
}

/// First timestamp in a media file = its offset from the segment start.
pub fn probe(path: &Path, field: &str) -> Result<f64> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", &format!("format={field}"), "-of", "csv=p=0"])
        .arg(path)
        .output()
        .context("running ffprobe")?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.trim().parse().with_context(|| format!("ffprobe {field} of {}: {text:?}", path.display()))
}

/// The recorded region in cursor coordinates.
fn region(src: &Sources) -> (f64, f64, f64, f64) {
    if let Some(w) = &src.window {
        // shortcut: the window's place at the start; moving it mid-recording misplaces the cursor.
        return (w.x, w.y, w.w, w.h);
    }
    let s = screens().into_iter().nth(src.display);
    let (x, y, w, h) = s.map_or((0.0, 0.0, 1920.0, 1080.0), |s| (s.x, s.y, s.w, s.h));
    match src.area {
        Some(a) => (x + a.x, y + a.y, a.w, a.h),
        None => (x, y, w, h),
    }
}

fn track_cursor(start: Instant, stop: Arc<AtomicBool>, (bx, by, bw, bh): (f64, f64, f64, f64)) -> CursorLog {
    let mut log = CursorLog::default();
    if stop.load(Ordering::Relaxed) {
        return log;
    }
    let Some(device) = DeviceState::checked_new() else {
        eprintln!("warning: cursor tracking unavailable (macOS: grant Accessibility permission; Linux: needs X11)");
        return log;
    };
    let (mut last, mut was_down) = ((i32::MIN, i32::MIN), false);
    while !stop.load(Ordering::Relaxed) {
        let m = device.get_mouse();
        let time_ms = start.elapsed().as_secs_f64() * 1000.0;
        if m.coords != last {
            last = m.coords;
            let (x, y) = ((m.coords.0 as f64 - bx) / bw, (m.coords.1 as f64 - by) / bh);
            log.moves.push(Move { cursor_id: "0".into(), time_ms, x, y });
        }
        let down = m.button_pressed.get(1).copied().unwrap_or(false);
        if down != was_down {
            was_down = down;
            log.clicks.push(Click { cursor_id: "0".into(), time_ms, down });
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    log
}

struct Running {
    dir: PathBuf,
    rel: String,
    caps: Vec<Capture>,
    stop_cursor: Arc<AtomicBool>,
    cursor: std::thread::JoinHandle<CursorLog>,
}

fn start_segment(root: &Path, index: usize, src: &Sources, preview: Option<&CameraFrame>) -> Result<Running> {
    let rel = format!("content/segments/segment-{index}");
    let dir = root.join(&rel);
    std::fs::create_dir_all(&dir)?;
    let screen = screens().into_iter().nth(src.display);
    // Area crop in physical pixels, even-sized for yuv420p.
    let crop = src.area.map(|a| {
        let s = screen.as_ref().map_or(1.0, |s| s.scale);
        let even = |v: f64| ((v * s) as u32) & !1;
        format!("crop={}:{}:{}:{}", even(a.w), even(a.h), even(a.x), even(a.y))
    });
    let start = Instant::now();
    let t0 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
    let stop_cursor = Arc::new(AtomicBool::new(src.camera_only || src.show_cursor));
    let (flag, bounds) = (stop_cursor.clone(), region(src));
    let cursor = std::thread::spawn(move || track_cursor(start, flag, bounds));

    let mut caps = Vec::new();
    let started = (|| -> Result<()> {
        match (&src.camera, src.camera_only) {
            (Some(dev), true) => caps.push(spawn(&dir, "display.mkv", Kind::Camera, device_input(Kind::Camera, dev), None, preview, t0, &src.quality)?),
            (None, true) => bail!("camera-only recording needs a camera"),
            _ => {
                let window = src.window.as_ref();
                caps.push(spawn(&dir, "display.mkv", Kind::Screen, screen_input(src.display, screen.as_ref(), window, src.fps, src.show_cursor), crop, None, t0, &src.quality)?);
                #[cfg(target_os = "macos")]
                attach_screencapturekit(caps.last_mut().expect("just pushed"), screen.as_ref(), window, src.fps, src.show_cursor, &src.exclude)?;
            }
        }
        if let (Some(dev), false) = (&src.camera, src.camera_only) {
            caps.push(spawn(&dir, "camera.mkv", Kind::Camera, device_input(Kind::Camera, dev), None, preview, t0, &src.quality)?);
        }
        if let Some(dev) = &src.mic {
            caps.push(spawn(&dir, "audio-input.mka", Kind::Mic, device_input(Kind::Mic, dev), None, None, t0, &src.quality)?);
        }
        if src.system_audio && !src.camera_only {
            caps.push(spawn(&dir, "system_audio.mka", Kind::Mic, system_audio_input()?, None, None, t0, &src.quality)?);
            #[cfg(target_os = "macos")]
            attach_system_audio(caps.last_mut().expect("just pushed"), screen.as_ref())?;
        }
        Ok(())
    })();
    if let Err(e) = started {
        for c in caps {
            let _ = stop(c);
        }
        stop_cursor.store(true, Ordering::Relaxed);
        return Err(e);
    }
    // Device errors (permissions, bad names) make ffmpeg exit right away.
    std::thread::sleep(Duration::from_millis(1500));
    for i in 0..caps.len() {
        if caps[i].child.try_wait()?.is_some() {
            let file = caps[i].file;
            for c in caps {
                let _ = stop(c);
            }
            stop_cursor.store(true, Ordering::Relaxed);
            bail!("ffmpeg could not capture {file}; see {}", dir.join(format!("{file}.log")).display());
        }
    }
    Ok(Running { dir, rel, caps, stop_cursor, cursor })
}

fn finish_segment(run: Running, fps: u32) -> Result<Segment> {
    let names: Vec<&str> = run.caps.iter().map(|c| c.file).collect();
    for c in run.caps {
        stop(c)?;
    }
    run.stop_cursor.store(true, Ordering::Relaxed);
    let cursor = run.cursor.join().unwrap_or_default();
    project::write(&run.dir.join("cursor.json"), &cursor)?;

    segment_of(&run.dir, &run.rel, fps, &|f| names.contains(&f))
}

/// Probes the tracks `has` reports in a segment folder; shared by the stop path and `recover`.
fn segment_of(dir: &Path, rel: &str, fps: u32, has: &dyn Fn(&str) -> bool) -> Result<Segment> {
    let track = |file: &str, fps: Option<f64>| -> Result<Option<Track>> {
        if !has(file) {
            return Ok(None);
        }
        let start_time = probe(&dir.join(file), "start_time")?;
        Ok(Some(Track { path: format!("{rel}/{file}"), fps, start_time }))
    };
    Ok(Segment {
        display: track("display.mkv", Some(fps as f64))?.context("display track missing")?,
        camera: track("camera.mkv", Some(30.0))?,
        mic: track("audio-input.mka", None)?,
        system_audio: track("system_audio.mka", None)?,
        cursor: Some(format!("{rel}/cursor.json")),
    })
}

/// Rebuilds the project of a recording that never finished from the Matroska tracks it left.
/// Nothing is written until every segment is probed, so a failure leaves the folder as it was.
pub fn recover(root: &Path) -> Result<()> {
    let segs = root.join("content/segments");
    let mut dirs: Vec<(usize, PathBuf)> = std::fs::read_dir(&segs)
        .with_context(|| format!("reading {}", segs.display()))?
        .flatten()
        .filter_map(|e| Some((e.file_name().to_str()?.strip_prefix("segment-")?.parse().ok()?, e.path())))
        .collect();
    dirs.sort();
    let mut meta = Meta { pretty_name: root.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() };
    let mut why = String::from("no segments found");
    for (i, dir) in dirs {
        // A track cut off before its first packet cannot be probed; drop it rather than the segment.
        let has = |f: &str| probe(&dir.join(f), "start_time").is_ok();
        // The cursor log is only written when a segment finishes; a crashed one has none.
        match segment_of(&dir, &format!("content/segments/segment-{i}"), 30, &has) {
            Ok(mut seg) => {
                if !dir.join("cursor.json").exists() {
                    seg.cursor = None;
                }
                meta.segments.push(seg)
            }
            Err(e) => why = format!("{e:#}"),
        }
    }
    if meta.segments.is_empty() {
        bail!("no playable display track ({why})");
    }
    let cfg = project::load_config(root, &meta)?;
    project::write(&root.join(project::META), &meta)?;
    project::write(&root.join(project::CONFIG), &cfg)
}

/// A recording in progress. Pausing closes the current segment; resuming starts the next.
pub struct Recorder {
    root: PathBuf,
    src: Sources,
    meta: Meta,
    current: Option<Running>,
    preview: Option<CameraFrame>,
}

impl Recorder {
    pub fn start(root: &Path, name: &str, src: Sources, preview: Option<CameraFrame>) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let current = Some(start_segment(root, 0, &src, preview.as_ref())?);
        let meta = Meta { pretty_name: name.into(), ..Default::default() };
        Ok(Self { root: root.to_path_buf(), src, meta, current, preview })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_paused(&self) -> bool {
        self.current.is_none()
    }

    pub fn pause(&mut self) -> Result<()> {
        if let Some(run) = self.current.take() {
            self.meta.segments.push(finish_segment(run, self.src.fps)?);
            // Saved after every segment so a crash keeps everything recorded so far.
            project::write(&self.root.join(project::META), &self.meta)?;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if self.current.is_none() {
            self.current = Some(start_segment(&self.root, self.meta.segments.len(), &self.src, self.preview.as_ref())?);
        }
        Ok(())
    }

    /// Finishes the recording and writes a default edit config; returns the project folder.
    pub fn stop(mut self) -> Result<PathBuf> {
        self.pause()?;
        project::write(&self.root.join(project::META), &self.meta)?;
        let cfg = project::load_config(&self.root, &self.meta)?;
        project::write(&self.root.join(project::CONFIG), &cfg)?;
        Ok(self.root)
    }

    /// Stops capturing and deletes everything recorded.
    pub fn discard(mut self) -> Result<()> {
        if let Some(run) = self.current.take() {
            let _ = finish_segment(run, self.src.fps);
        }
        std::fs::remove_dir_all(&self.root).context("deleting the recording")
    }
}

/// Terminal recorder: records until the user types `q`; Enter toggles pause/resume.
pub fn record(root: &Path, name: &str, src: Sources) -> Result<()> {
    let mut rec = Recorder::start(root, name, src, None)?;
    println!("● recording to {}\n  Enter = pause/resume, q + Enter = stop", root.display());
    for line in std::io::stdin().lock().lines() {
        if line?.trim() == "q" {
            break;
        }
        if rec.is_paused() {
            rec.resume()?;
            println!("● resumed");
        } else {
            rec.pause()?;
            println!("❚❚ paused (Enter = resume, q + Enter = stop)");
        }
    }
    let root = rec.stop()?;
    println!("■ saved {}", root.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mjpeg_stream_yields_whole_jpegs() {
        let out = Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=size=640x480:rate=30", "-frames:v", "10"])
            .args(PREVIEW_OUT)
            .output()
            .expect("ffmpeg");
        let frame: CameraFrame = Default::default();
        pump_mjpeg(std::io::Cursor::new(out.stdout), frame.clone());
        std::thread::sleep(Duration::from_millis(300));
        let jpeg = frame.lock().unwrap().clone();
        assert_eq!((&jpeg[..2], &jpeg[jpeg.len() - 2..]), (&[0xFF, 0xD8][..], &[0xFF, 0xD9][..]));
        let img = image::load_from_memory(&jpeg).expect("decodes");
        assert_eq!((img.width(), img.height()), (360, 360));
    }

    #[cfg(unix)]
    #[test]
    fn stop_all_children_kills_what_ignores_sigint() {
        let mut child = Command::new("sh").args(["-c", "trap '' INT; sleep 30"]).spawn().unwrap();
        track(&child, true);
        std::thread::sleep(Duration::from_millis(200));
        stop_all_children();
        std::thread::sleep(Duration::from_millis(100));
        assert!(child.try_wait().unwrap().is_some(), "child still running");
    }

    /// Live checks against the real machine (screen, speakers, mic); run with `--ignored`.
    #[test]
    #[ignore]
    fn live_windows_and_screenshots() {
        let wins = windows();
        assert!(!wins.is_empty(), "no windows listed (Screen Recording permission?)");
        assert!(wins.iter().all(|w| w.w >= 50.0 && w.h >= 50.0));
        let dir = std::env::temp_dir();
        let shot = dir.join("precap-live-display.png");
        screenshot(0, None, None, &[], &shot, None).unwrap();
        let img = image::open(&shot).unwrap();
        let s = &screens()[0];
        assert_eq!((img.width() as usize, img.height() as usize), sck_size(s));
        let area = dir.join("precap-live-area.png");
        screenshot(0, Some(Area { x: 10.0, y: 20.0, w: 300.0, h: 200.0 }), None, &[], &area, None).unwrap();
        let img = image::open(&area).unwrap();
        assert_eq!((img.width() as f64, img.height() as f64), (300.0 * s.scale, 200.0 * s.scale));
        let thumb = dir.join("precap-live-window.jpg");
        screenshot(wins[0].display, None, Some(&wins[0]), &[], &thumb, Some(480)).unwrap();
        assert!(image::open(&thumb).unwrap().width() <= 480);
        for f in [shot, area, thumb] {
            std::fs::remove_file(f).unwrap();
        }
    }

    #[test]
    #[ignore]
    fn live_system_audio_records_what_plays() {
        let root = std::env::temp_dir().join(format!("precap-live-sys-{}", std::process::id()));
        let src = Sources { display: 0, area: None, window: None, camera: None, mic: None, system_audio: true, fps: 30, camera_only: false, show_cursor: false, exclude: vec![], quality: "balanced".into() };
        let rec = Recorder::start(&root, "live", src, None).unwrap();
        let _ = Command::new("afplay").args(["-v", "0.3", "/System/Library/Sounds/Glass.aiff"]).status();
        let _ = Command::new("afplay").args(["-v", "0.3", "/System/Library/Sounds/Glass.aiff"]).status();
        rec.stop().unwrap();
        let meta: Meta = project::read(&root.join(project::META)).unwrap();
        let sys = meta.segments[0].system_audio.as_ref().expect("system audio track");
        let out = Command::new("ffmpeg").args(["-hide_banner", "-i"]).arg(root.join(&sys.path)).args(["-af", "volumedetect", "-f", "null", "-"]).output().unwrap();
        let log = String::from_utf8_lossy(&out.stderr);
        let max: f64 = log.split("max_volume: ").nth(1).and_then(|r| r.split(' ').next()).and_then(|v| v.parse().ok()).expect("volumedetect output");
        std::fs::remove_dir_all(&root).unwrap();
        assert!(max > -40.0, "system audio is silent (max {max} dB)");
    }

    #[test]
    #[ignore]
    fn live_window_recording_to_instant_mp4() {
        let win = windows().into_iter().next().expect("a window");
        let root = std::env::temp_dir().join(format!("precap-live-win-{}", std::process::id()));
        let src = Sources { display: win.display, area: None, window: Some(win.clone()), camera: None, mic: None, system_audio: false, fps: 30, camera_only: false, show_cursor: true, exclude: vec![], quality: "balanced".into() };
        let rec = Recorder::start(&root, "live", src, None).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        rec.stop().unwrap();
        let out = crate::instant::finish(&root, None, 3840).unwrap();
        let (w, h) = crate::export::probe_size(&out).unwrap();
        let screen = screens().into_iter().nth(win.display);
        assert_eq!((w, h), capture_size(screen.as_ref(), Some(&win)));
        assert!(probe(&out, "duration").unwrap() > 2.5, "covers the whole take");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore]
    fn live_mic_meter_reports_levels() {
        let dev = devices().unwrap().mics.into_iter().find(|m| m.name.contains("MacBook")).expect("built-in mic").id;
        let levels: Arc<Mutex<Vec<f64>>> = Default::default();
        let sink = levels.clone();
        let meter = MicMeter::start(&dev, move |db| sink.lock().unwrap().push(db)).unwrap();
        std::thread::sleep(Duration::from_millis(2500));
        drop(meter);
        let levels = levels.lock().unwrap();
        assert!(levels.len() >= 25, "only {} readings in 2.5 s", levels.len());
        assert!(levels.iter().all(|l| (-60.0..=0.0).contains(l)));
    }

    #[test]
    fn mic_levels_in_dbfs() {
        assert!(level_db(32767).abs() < 0.001);
        assert!((level_db(3277) - -20.0).abs() < 0.01);
        assert_eq!((level_db(0), level_db(1)), (-60.0, -60.0));
    }

    #[test]
    fn picks_camera_mode_closest_to_30fps() {
        let text = "[in#0 @ 0x1] Selected framerate (30.000000) is not supported by the device.\n[in#0 @ 0x1] Supported modes:\n[in#0 @ 0x1]   1920x1080@[60.000000 60.000000]fps\n[in#0 @ 0x1]   1280x720@[24.000000 24.000000]fps\n";
        assert_eq!(parse_modes(text), Some(("1280x720".into(), "24".into())));
        assert_eq!(parse_modes("[in#0] all good"), None);
    }

    #[test]
    fn parses_device_lists() {
        let mac = "[AVFoundation indev @ 0x1] AVFoundation video devices:\n[AVFoundation indev @ 0x1] [0] MacBook Air Camera\n[AVFoundation indev @ 0x1] [1] Capture screen 0\n[AVFoundation indev @ 0x1] AVFoundation audio devices:\n[AVFoundation indev @ 0x1] [0] MacBook Air Microphone\n";
        let (cams, mics) = parse_avfoundation(mac);
        assert_eq!((cams.len(), cams[0].id.as_str(), cams[0].name.as_str()), (1, "MacBook Air Camera", "MacBook Air Camera"));
        let (cams, _) = parse_avfoundation("[AVFoundation indev @ 0x1] [3] Jo's iPhone: Camera\n");
        assert_eq!(cams[0].id, "3", "a ':' in the name falls back to the index");
        assert_eq!(mics[0].name, "MacBook Air Microphone");

        let win = "[dshow @ 01] \"Integrated Camera\" (video)\n[dshow @ 01]   Alternative name \"@device_pnp_x\"\n[dshow @ 01] \"Microphone (Realtek)\" (audio)\n";
        let (cams, mics) = parse_dshow(win);
        assert_eq!((cams[0].id.as_str(), mics[0].id.as_str()), ("Integrated Camera", "Microphone (Realtek)"));

        let linux = "Auto-detected sources for pulse:\n* alsa_input.pci.analog-stereo [Built-in Audio]\n  alsa_output.pci.monitor [Monitor of Built-in]\n";
        let srcs = parse_sources(linux);
        assert_eq!((srcs.len(), srcs[0].id.as_str(), srcs[0].name.as_str()), (2, "alsa_input.pci.analog-stereo", "Built-in Audio"));
    }

    fn clip(path: &Path, args: &str) {
        let mut a: Vec<String> = ["-v", "error", "-y"].map(String::from).to_vec();
        a.extend(args.split(' ').map(String::from));
        a.push(path.display().to_string());
        assert!(Command::new("ffmpeg").args(a).status().unwrap().success());
    }

    /// A folder with only segment tracks (no meta/config/cursor log), as a crash leaves it.
    fn interrupted(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("precap {name} {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for i in 0..2 {
            std::fs::create_dir_all(root.join(format!("content/segments/segment-{i}"))).unwrap();
        }
        root
    }

    #[test]
    fn recovers_interrupted_recording() {
        let root = interrupted("recover");
        let seg = |i: usize, f: &str| root.join(format!("content/segments/segment-{i}")).join(f);
        assert!(project::incomplete(&root));
        clip(&seg(0, "display.mkv"), "-f lavfi -i testsrc=s=320x240:r=30:d=2 -c:v libx264 -pix_fmt yuv420p");
        clip(&seg(0, "audio-input.mka"), "-f lavfi -i sine=f=440:d=2 -c:a aac");
        clip(&seg(1, "display.mkv"), "-f lavfi -i testsrc=s=320x240:r=30:d=1 -c:v libx264 -pix_fmt yuv420p");
        std::fs::write(seg(1, "camera.mkv"), b"cut off before any packet").unwrap();
        recover(&root).unwrap();
        assert!(!project::incomplete(&root));
        let meta: Meta = project::read(&root.join(project::META)).unwrap();
        assert_eq!(meta.segments.len(), 2);
        assert!(meta.segments[0].mic.is_some() && meta.segments[0].cursor.is_none());
        assert!(meta.segments[1].camera.is_none(), "unreadable optional track is dropped");
        let cfg: project::Config = project::read(&root.join(project::CONFIG)).unwrap();
        let d: Vec<f64> = cfg.timeline.segments.iter().map(|c| c.end).collect();
        assert!((d[0] - 2.0).abs() < 0.2 && (d[1] - 1.0).abs() < 0.2, "{d:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn recovery_without_display_leaves_files_alone() {
        let root = interrupted("norecover");
        let audio = root.join("content/segments/segment-0/audio-input.mka");
        clip(&audio, "-f lavfi -i sine=f=440:d=1 -c:a aac");
        std::fs::write(root.join("content/segments/segment-1/display.mkv"), b"junk").unwrap();
        let before = std::fs::read(&audio).unwrap();
        let e = recover(&root).unwrap_err().to_string();
        assert!(e.contains("no playable display track"), "{e}");
        assert!(project::incomplete(&root) && !root.join(project::META).exists());
        assert_eq!(std::fs::read(&audio).unwrap(), before);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
