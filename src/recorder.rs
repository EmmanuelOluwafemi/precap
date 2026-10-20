//! The recording app: Cap-style windows for setup (main), the target overlay, the floating
//! camera bubble, the recording controls bar, the teleprompter and the Instant result card.
//! Front end pages live in ui/ (recorder, target, controls, camera, teleprompter, overlay).

use crate::record::{self, Area, CameraFrame, CameraPreview, MicMeter, Recorder, Sources};
use crate::settings::{self, Hotkey, Settings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

/// What the target overlay chose; camera, mic and system audio come from the main window.
#[derive(Deserialize, Clone)]
struct Target {
    display: usize,
    area: Option<Area>,
    #[serde(default)]
    window: Option<u32>,
    camera_only: bool,
}

/// The camera bubble's look, reported by its page (baked into Instant recordings).
#[derive(Deserialize, Clone)]
struct CameraStyle {
    shape: String,
    mirror: bool,
}

impl Default for CameraStyle {
    fn default() -> Self {
        Self { shape: "round".into(), mirror: false }
    }
}

#[derive(Default)]
struct RecState {
    frame: CameraFrame,
    preview: Mutex<Option<CameraPreview>>,
    meter: Mutex<Option<MicMeter>>,
    camera: Mutex<Option<String>>,
    mic: Mutex<Option<String>>,
    system_audio: Mutex<bool>,
    camera_style: Mutex<CameraStyle>,
    target: Mutex<Option<Target>>,
    /// "studio" or "instant" for the recording being set up or running.
    mode: Mutex<String>,
    rec: Mutex<Option<Recorder>>,
    settings: Mutex<Settings>,
    /// Registered global shortcuts and the action each one runs.
    hotkeys: Mutex<Vec<(Shortcut, String)>>,
}

fn current(st: &RecState) -> Settings {
    lock(&st.settings).clone()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[tauri::command]
fn devices() -> CmdResult<record::Devices> {
    record::devices().map_err(err)
}

#[tauri::command]
async fn windows() -> Vec<record::Window> {
    record::windows()
}

fn find_window(id: u32) -> CmdResult<record::Window> {
    record::windows().into_iter().find(|w| w.id == id).ok_or_else(|| "That window is no longer open".into())
}

#[tauri::command]
async fn window_thumbnail(id: u32) -> CmdResult<Response> {
    let w = find_window(id)?;
    let tmp = crate::os::temp_file("jpg").map_err(err)?;
    record::screenshot(w.display, None, Some(&w), &[], &tmp, Some(480)).map_err(err)?;
    let bytes = std::fs::read(&tmp).map_err(err)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(Response::new(bytes))
}

/// Selects the camera for the bubble (None hides it) and starts its live preview.
#[tauri::command]
fn set_camera(app: AppHandle, st: State<RecState>, id: Option<String>) -> CmdResult<()> {
    *lock(&st.preview) = None;
    lock(&st.frame).clear();
    *lock(&st.camera) = id.clone();
    match id {
        Some(id) => {
            *lock(&st.preview) = Some(CameraPreview::start(&id, st.frame.clone()).map_err(err)?);
            camera_window(&app).map_err(err)?.show().map_err(err)
        }
        None => {
            if let Some(w) = app.get_webview_window("camera") {
                w.close().map_err(err)?;
            }
            Ok(())
        }
    }
}

/// Selects the mic and runs its level meter (`audio-input-level-change` events, dBFS).
#[tauri::command]
fn set_mic(app: AppHandle, st: State<RecState>, id: Option<String>) -> CmdResult<()> {
    *lock(&st.meter) = None;
    *lock(&st.mic) = id.clone();
    match id {
        Some(id) => {
            let emitter = app.clone();
            *lock(&st.meter) = Some(MicMeter::start(&id, move |db| drop(emitter.emit("audio-input-level-change", db))).map_err(err)?);
        }
        None => app.emit("audio-input-level-change", Option::<f64>::None).map_err(err)?,
    }
    Ok(())
}

#[tauri::command]
fn set_system_audio(st: State<RecState>, on: bool) {
    *lock(&st.system_audio) = on;
}

#[tauri::command]
fn camera_style(st: State<RecState>, shape: String, mirror: bool) {
    *lock(&st.camera_style) = CameraStyle { shape, mirror };
}

/// Current picks, so every window (main, overlay) shows the same camera/mic/system audio.
#[tauri::command]
fn selection(st: State<RecState>) -> (Option<String>, Option<String>, bool) {
    (lock(&st.camera).clone(), lock(&st.mic).clone(), *lock(&st.system_audio))
}

/// Newest camera JPEG (empty until the first frame arrives).
#[tauri::command]
fn camera_frame(st: State<RecState>) -> Response {
    Response::new(lock(&st.frame).clone())
}

/// Opens Cap-style target selection: a full-screen overlay on `display`
/// (`kind` = "display" | "window" | "area" | "camera") holding the Start panel for `mode`.
#[tauri::command]
fn open_target(app: AppHandle, kind: String, display: usize, mode: Option<String>) -> CmdResult<()> {
    let s = record::screens().into_iter().nth(display).ok_or("no such display")?;
    if let Some(w) = app.get_webview_window("target") {
        w.close().map_err(err)?;
    }
    // Window positions are logical points; screens() reports pixels except on macOS.
    let k = if cfg!(target_os = "macos") { 1.0 } else { app.primary_monitor().ok().flatten().map_or(1.0, |m| m.scale_factor()) };
    let mode = mode.unwrap_or_else(|| "studio".into());
    let page = format!("target.html?kind={kind}&display={display}&w={}&h={}&name={}&mode={mode}", s.w, s.h, s.name.replace(['&', '?', '#'], " "));
    let w = overlay(&app, "target", &page, s.w / k, s.h / k).map_err(err)?.position(s.x / k, s.y / k).build().map_err(err)?;
    w.set_focus().map_err(err)
}

/// Called by the overlay's Start Recording: hides setup windows and shows the
/// controls bar, which runs the countdown and then calls `start_recording`.
#[tauri::command]
fn prepare_recording(app: AppHandle, st: State<RecState>, target: Target, countdown: u32, mode: Option<String>) -> CmdResult<()> {
    prepare(&app, &st, target, countdown, mode.as_deref().unwrap_or("studio")).map_err(err)
}

fn prepare(app: &AppHandle, st: &RecState, target: Target, countdown: u32, mode: &str) -> anyhow::Result<()> {
    let mode = if mode == "instant" { "instant" } else { "studio" };
    *lock(&st.target) = Some(target);
    *lock(&st.mode) = mode.into();
    if let Some(w) = app.get_webview_window("target") {
        w.close()?;
    }
    if let Some(w) = app.get_webview_window("main") {
        // Cap's "Main window when recording starts" setting.
        if current(st).main_window_recording_start_behaviour == "minimise" { w.minimize()? } else { w.hide()? }
    }
    controls_window(app, countdown, mode)?.show()?;
    Ok(())
}

fn app_data(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?)
}

fn data_dir(app: &AppHandle, kind: &str) -> anyhow::Result<PathBuf> {
    if kind == "recordings" {
        if let Some(p) = current(&app.state::<RecState>()).recordings_path {
            return Ok(PathBuf::from(p));
        }
    }
    Ok(app_data(app)?.join(kind))
}

/// Project name from the "Default project name" template, e.g. "Built-in Retina Display (Display) 2026-10-20 03.15 PM".
fn project_name(app: &AppHandle, t: &Target, window: Option<&record::Window>, mode: &str) -> String {
    let screen = record::screens().into_iter().nth(t.display).map_or("Screen".into(), |d| d.name);
    let (source, kind) = match (t.camera_only, window, t.area.is_some()) {
        (true, ..) => ("Camera".to_string(), "Camera"),
        (_, Some(w), _) => (if w.title.is_empty() { w.app.clone() } else { w.title.clone() }, "Window"),
        (_, None, true) => (screen, "Area"),
        _ => (screen, "Display"),
    };
    let mode = match mode {
        "instant" => "Instant",
        "screenshot" => "Screenshot",
        _ => "Studio",
    };
    let template = current(&app.state::<RecState>()).default_project_name_template.unwrap_or_else(|| settings::DEFAULT_NAME_TEMPLATE.into());
    let now = chrono::Local::now();
    settings::format_name(&template, &source, kind, mode, now)
        .ok()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| settings::format_name(settings::DEFAULT_NAME_TEMPLATE, &source, kind, mode, now).unwrap_or_default())
}

/// Windows the user excluded in settings that are on screen now (macOS can leave them out).
fn excluded_windows(st: &RecState) -> Vec<u32> {
    let rules = current(st).excluded_windows;
    if rules.is_empty() {
        return Vec::new();
    }
    record::windows().into_iter().filter(|w| rules.iter().any(|r| r.matches(w))).map(|w| w.id).collect()
}

fn begin(app: &AppHandle, st: &RecState) -> anyhow::Result<()> {
    let t = lock(&st.target).clone().ok_or_else(|| anyhow::anyhow!("no recording target"))?;
    let window = t.window.map(find_window).transpose().map_err(|e| anyhow::anyhow!(e))?;
    let (camera, mic) = (lock(&st.camera).clone(), lock(&st.mic).clone());
    // The recording takes over the camera and keeps feeding the bubble from its own ffmpeg.
    *lock(&st.preview) = None;
    let mode = lock(&st.mode).clone();
    let root = crate::os::unique(&data_dir(app, "recordings")?, &project_name(app, &t, window.as_ref(), &mode), "cap");
    let name = root.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let preview = camera.as_ref().map(|_| st.frame.clone());
    let instant = mode == "instant";
    let system_audio = *lock(&st.system_audio) && !t.camera_only;
    let s = current(st);
    let exclude = excluded_windows(st);
    let src = Sources { display: t.display, area: t.area, window, camera, mic, system_audio, fps: s.max_fps, camera_only: t.camera_only, show_cursor: instant, exclude, quality: s.studio_recording_quality };
    *lock(&st.rec) = Some(Recorder::start(&root, &name, src, preview)?);
    let _ = app.emit("recording-event", "Started");
    Ok(())
}

#[tauri::command]
async fn start_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<()> {
    begin(&app, &st).map_err(|e| {
        // The controls bar closes, so the main window shows the error.
        let _ = back_to_setup(&app, &st);
        let _ = app.emit_to("main", "recording-error", err(&e));
        err(e)
    })
}

#[tauri::command]
async fn pause_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<()> {
    lock(&st.rec).as_mut().ok_or("not recording")?.pause().map_err(err)?;
    app.emit("recording-event", "Paused").map_err(err)
}

#[tauri::command]
async fn resume_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<()> {
    lock(&st.rec).as_mut().ok_or("not recording")?.resume().map_err(err)?;
    app.emit("recording-event", "Resumed").map_err(err)
}

#[derive(Serialize)]
struct RecordingState {
    recording: bool,
    paused: bool,
}

#[tauri::command]
fn recording_state(st: State<RecState>) -> RecordingState {
    let rec = lock(&st.rec);
    RecordingState { recording: rec.is_some(), paused: rec.as_ref().is_some_and(|r| r.is_paused()) }
}

/// Where the camera bubble sits within the recorded picture, as fractions, from the
/// camera window's place on screen (the bubble is the window minus its 56 pt toolbar).
fn bubble(app: &AppHandle, st: &RecState, t: &Target) -> Option<crate::instant::Bubble> {
    let cam = app.get_webview_window("camera")?;
    // Screen coordinates are points on macOS and pixels elsewhere, like record::screens().
    let k = if cfg!(target_os = "macos") { cam.scale_factor().ok()? } else { 1.0 };
    let (pos, size) = (cam.outer_position().ok()?, cam.inner_size().ok()?);
    let toolbar = 56.0 * cam.scale_factor().ok()? / k;
    let (bx, by, bw, bh) = (pos.x as f64 / k, pos.y as f64 / k + toolbar, size.width as f64 / k, size.height as f64 / k - toolbar);
    let s = record::screens().into_iter().nth(t.display)?;
    let (rx, ry, rw, rh) = match (t.window.and_then(|id| find_window(id).ok()), t.area) {
        (Some(w), _) => (w.x, w.y, w.w, w.h),
        (None, Some(a)) => (s.x + a.x, s.y + a.y, a.w, a.h),
        (None, None) => (s.x, s.y, s.w, s.h),
    };
    let style = lock(&st.camera_style).clone();
    let b = crate::instant::Bubble { x: (bx - rx) / rw, y: (by - ry) / rh, w: bw / rw, h: bh / rh, round: style.shape == "round", mirror: style.mirror };
    // Keep it fully in the picture (the bubble may sit outside a window or area).
    Some(crate::instant::Bubble { x: b.x.clamp(0.0, (1.0 - b.w).max(0.0)), y: b.y.clamp(0.0, (1.0 - b.h).max(0.0)), ..b })
}

/// Studio: opens the editor. Instant: finishes the mp4, copies it to the clipboard and shows the result card.
#[tauri::command]
async fn stop_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<String> {
    let rec = lock(&st.rec).take().ok_or("not recording")?;
    let root = rec.stop().map_err(err)?;
    let _ = app.emit("recording-stopped", ());
    let target = lock(&st.target).clone();
    let result = if *lock(&st.mode) == "instant" {
        let b = target.as_ref().filter(|t| !t.camera_only).and_then(|t| bubble(&app, &st, t));
        let out = crate::instant::finish(&root, b, current(&st).instant_mode_max_resolution as usize).map_err(err);
        back_to_setup(&app, &st).map_err(err)?;
        let out = out?;
        if let Err(e) = crate::os::copy_file(&out) {
            eprintln!("copying the video to the clipboard: {e:#}");
        }
        overlay_window(&app, &root).map_err(err)?;
        root
    } else {
        let s = current(&st);
        if s.auto_zoom_on_clicks {
            if let Err(e) = auto_zoom(&root, s.default_zoom_amount) {
                eprintln!("auto zoom: {e:#}");
            }
        }
        // Cap's "After a Studio recording" setting.
        if s.post_studio_recording_behaviour == "showOverlay" {
            overlay_window(&app, &root).map_err(err)?;
        } else {
            open_editor(root.display().to_string())?;
        }
        back_to_setup(&app, &st).map_err(err)?;
        root
    };
    Ok(result.display().to_string())
}

/// Cap's "Auto zoom on clicks": zoom segments around the recorded clicks, written into the new project.
fn auto_zoom(root: &Path, amount: f64) -> anyhow::Result<()> {
    let p = crate::export::Project::load(root)?;
    let duration = p.cfg.timeline.segments.iter().map(|c| (c.end - c.start) / c.timescale).sum();
    let clicks = crate::autozoom::click_times(&p.meta, &p.cfg, &p.cursors);
    let mut cfg = p.cfg;
    cfg.timeline.zoom_segments = crate::autozoom::zoom_segments_from_clicks(&clicks, duration, amount);
    crate::project::write(&root.join(crate::project::CONFIG), &cfg)
}

/// Throws the current take away and starts a fresh one with the same setup.
#[tauri::command]
async fn restart_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<()> {
    if let Some(rec) = lock(&st.rec).take() {
        rec.discard().map_err(err)?;
    }
    start_recording(app, st).await
}

/// Stops and deletes the current recording.
#[tauri::command]
async fn delete_recording(app: AppHandle, st: State<'_, RecState>) -> CmdResult<()> {
    if let Some(rec) = lock(&st.rec).take() {
        rec.discard().map_err(err)?;
        let _ = app.emit("recording-stopped", ());
    }
    back_to_setup(&app, &st).map_err(err)
}

/// Closes the controls bar, shows the setup window again and resumes the camera preview.
fn back_to_setup(app: &AppHandle, st: &RecState) -> anyhow::Result<()> {
    if let Some(w) = app.get_webview_window("controls") {
        w.close()?;
    }
    if let Some(w) = app.get_webview_window("main") {
        w.unminimize()?;
        w.show()?;
        w.set_focus()?;
    }
    if let Some(id) = lock(&st.camera).clone() {
        *lock(&st.preview) = Some(CameraPreview::start(&id, st.frame.clone())?);
    }
    Ok(())
}

/// Takes a screenshot of the target (our own windows left out), saves it as a project
/// and opens the screenshot editor on it.
#[tauri::command]
async fn take_screenshot(app: AppHandle, target: Target) -> CmdResult<String> {
    if let Some(w) = app.get_webview_window("target") {
        w.close().map_err(err)?;
    }
    let shot = || -> CmdResult<PathBuf> {
        let window = target.window.map(find_window).transpose()?;
        let dir = data_dir(&app, "screenshots").map_err(err)?;
        std::fs::create_dir_all(&dir).map_err(err)?;
        let exclude = excluded_windows(&app.state::<RecState>());
        let name = project_name(&app, &target, window.as_ref(), "screenshot");
        let root = crate::screenshot::create(&dir, &name, |png| record::screenshot(target.display, target.area, window.as_ref(), &exclude, png, None)).map_err(err)?;
        open_screenshot(root.display().to_string())?;
        Ok(root)
    };
    // The overlay is gone by now, so the main window shows any error.
    shot().map(|root| root.display().to_string()).inspect_err(|e| drop(app.emit_to("main", "recording-error", format!("Failed to take screenshot: {e}"))))
}

#[derive(Serialize)]
struct RecordingItem {
    name: String,
    path: String,
    /// Seconds since the Unix epoch.
    modified: u64,
}

/// Project folders under `kind` holding `marker`, newest first.
fn list_projects(app: &AppHandle, kind: &str, marker: &str) -> CmdResult<Vec<RecordingItem>> {
    let dir = data_dir(app, kind).map_err(err)?;
    let mut items: Vec<RecordingItem> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.path().join(marker).exists())
        .map(|e| {
            let modified = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            let name = e.path().file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            RecordingItem { name, path: e.path().display().to_string(), modified }
        })
        .collect();
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(items)
}

#[tauri::command]
fn recordings(app: AppHandle) -> CmdResult<Vec<RecordingItem>> {
    list_projects(&app, "recordings", crate::project::META)
}

#[tauri::command]
fn screenshots(app: AppHandle) -> CmdResult<Vec<RecordingItem>> {
    list_projects(&app, "screenshots", crate::screenshot::ORIGINAL)
}

#[tauri::command]
async fn screenshot_thumbnail(path: String) -> CmdResult<Response> {
    crate::screenshot::thumbnail(Path::new(&path), 480).map(Response::new).map_err(err)
}

/// Each editor runs as its own process (`precap edit|screenshot <project>`).
fn spawn_self(cmd: &str, path: &str) -> CmdResult<()> {
    let exe = std::env::current_exe().map_err(err)?;
    std::process::Command::new(exe).arg(cmd).arg(Path::new(path)).spawn().map(|_| ()).map_err(err)
}

#[tauri::command]
fn open_editor(path: String) -> CmdResult<()> {
    spawn_self("edit", &path)
}

#[tauri::command]
fn open_screenshot(path: String) -> CmdResult<()> {
    spawn_self("screenshot", &path)
}

#[tauri::command]
fn open_teleprompter(app: AppHandle) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window("teleprompter") {
        w.unminimize().map_err(err)?;
        w.show().map_err(err)?;
        return w.set_focus().map_err(err);
    }
    let b = WebviewWindowBuilder::new(&app, "teleprompter", WebviewUrl::App("teleprompter.html".into()))
        .title("Precap Teleprompter")
        .inner_size(560.0, 320.0)
        .min_inner_size(420.0, 220.0)
        .center()
        .transparent(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        // Like Cap's excluded "Cap Teleprompter" window: never in the recording.
        .content_protected(true);
    #[cfg(target_os = "macos")]
    let b = b.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(14.0, 14.0));
    #[cfg(not(target_os = "macos"))]
    let b = b.decorations(false);
    b.build().map_err(err)?.set_focus().map_err(err)
}

// ---------- Instant result card ----------

#[derive(Serialize)]
struct InstantInfo {
    name: String,
    duration: f64,
    size: u64,
    output: String,
}

fn output_of(path: &str) -> PathBuf {
    Path::new(path).join(crate::instant::OUTPUT)
}

fn meta_of(path: &str) -> CmdResult<crate::project::Meta> {
    crate::project::read(&Path::new(path).join(crate::project::META)).map_err(err)
}

/// The bundle's shareable mp4: Instant wrote it at stop; a Studio recording is rendered now
/// with its edit (Cap's overlay renders on Copy/Export too).
fn ensure_output(path: &str) -> CmdResult<PathBuf> {
    let out = output_of(path);
    if !out.exists() {
        let p = crate::export::Project::load(Path::new(path)).map_err(err)?;
        crate::export::render(&p, 30, 1080, crate::export::Target::Video(out.clone(), 20), &mut |_, _| {}).map_err(err)?;
    }
    Ok(out)
}

#[tauri::command]
async fn instant_info(path: String) -> CmdResult<InstantInfo> {
    let out = output_of(&path);
    let name = Path::new(&path).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if out.exists() {
        let size = std::fs::metadata(&out).map_err(err)?.len();
        return Ok(InstantInfo { name, duration: record::probe(&out, "duration").unwrap_or(0.0), size, output: out.display().to_string() });
    }
    // A Studio recording not rendered yet: its raw tracks.
    let meta = meta_of(&path)?;
    let files = meta.segments.iter().flat_map(|s| [Some(&s.display), s.camera.as_ref(), s.mic.as_ref(), s.system_audio.as_ref()]).flatten();
    let size = files.filter_map(|t| std::fs::metadata(Path::new(&path).join(&t.path)).ok()).map(|m| m.len()).sum();
    let duration = meta.segments.iter().filter_map(|s| record::probe(&Path::new(&path).join(&s.display.path), "duration").ok()).sum();
    Ok(InstantInfo { name, duration, size, output: path })
}

#[tauri::command]
async fn instant_thumbnail(path: String) -> CmdResult<Response> {
    let out = output_of(&path);
    let src = if out.exists() { out } else { Path::new(&path).join(&meta_of(&path)?.segments.first().ok_or("empty recording")?.display.path) };
    let out = std::process::Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(src).args(["-frames:v", "1", "-vf", "scale=520:-2", "-f", "mjpeg", "-"]).output().map_err(err)?;
    if out.stdout.is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(Response::new(out.stdout))
}

#[tauri::command]
async fn copy_video(path: String) -> CmdResult<()> {
    crate::os::copy_file(&ensure_output(&path)?).map_err(err)
}

#[tauri::command]
async fn export_video(path: String) -> CmdResult<String> {
    let name = Path::new(&path).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Recording".into());
    let dest = crate::os::unique(&crate::os::media_dir(false), &name, "mp4");
    std::fs::copy(ensure_output(&path)?, &dest).map_err(err)?;
    Ok(dest.display().to_string())
}

#[tauri::command]
fn reveal(path: String) -> CmdResult<()> {
    crate::os::reveal(Path::new(&path)).map_err(err)
}

#[tauri::command]
fn close_overlay(app: AppHandle) -> CmdResult<()> {
    match app.get_webview_window("recordings-overlay") {
        Some(w) => w.close().map_err(err),
        None => Ok(()),
    }
}

// ---------- settings and global shortcuts ----------

#[tauri::command]
fn get_settings(st: State<RecState>) -> Settings {
    current(&st)
}

/// Merges `patch` into the settings, saves them and tells every window.
#[tauri::command]
fn set_settings(app: AppHandle, st: State<RecState>, patch: serde_json::Value) -> CmdResult<Settings> {
    let old = current(&st);
    let new = settings::merged(&old, patch).map_err(err)?;
    if new.hotkeys != old.hotkeys {
        if let Err(e) = register_hotkeys(&app, &new.hotkeys) {
            let _ = register_hotkeys(&app, &old.hotkeys);
            return Err(format!("That shortcut can't be used: {e}"));
        }
    }
    settings::save(&app_data(&app).map_err(err)?, &new).map_err(err)?;
    *lock(&st.settings) = new.clone();
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

#[tauri::command]
async fn pick_recordings_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().set_title("Choose Recordings Folder").blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[tauri::command]
fn default_recordings_folder(app: AppHandle) -> CmdResult<String> {
    Ok(app_data(&app).map_err(err)?.join("recordings").display().to_string())
}

/// Live preview of a project name template, with a sample display recording.
#[tauri::command]
fn format_project_name(template: String) -> String {
    settings::format_name(&template, "Built-in Retina Display", "Display", "Studio", chrono::Local::now()).unwrap_or_else(|e| format!("{e:#}"))
}

#[tauri::command]
fn os() -> &'static str {
    std::env::consts::OS
}

#[tauri::command]
fn open_settings(app: AppHandle, page: Option<String>) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window("settings") {
        if let Some(page) = page {
            let _ = w.emit("settings-page", page);
        }
        w.unminimize().map_err(err)?;
        w.show().map_err(err)?;
        return w.set_focus().map_err(err);
    }
    let url = format!("settings.html?page={}", page.as_deref().unwrap_or("general"));
    let b = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App(url.into()))
        .title("Precap Settings")
        .inner_size(760.0, 560.0)
        .min_inner_size(640.0, 480.0)
        .theme(Some(tauri::Theme::Dark))
        // Cap excludes "Cap Settings" from recordings by default.
        .content_protected(true);
    #[cfg(target_os = "macos")]
    let b = b.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(16.0, 18.0));
    b.build().map_err(err)?.set_focus().map_err(err)
}

/// A browser `KeyboardEvent.code` chord as an OS shortcut; bare keys only for F-keys.
fn shortcut_of(h: &Hotkey) -> Option<Shortcut> {
    let code: Code = h.code.parse().ok()?;
    let mut mods = Modifiers::empty();
    for (on, m) in [(h.ctrl, Modifiers::CONTROL), (h.shift, Modifiers::SHIFT), (h.alt, Modifiers::ALT), (h.meta, Modifiers::SUPER)] {
        if on {
            mods |= m;
        }
    }
    let fkey = h.code.strip_prefix('F').is_some_and(|n| n.parse::<u8>().is_ok());
    (!mods.is_empty() || fkey).then(|| Shortcut::new(Some(mods).filter(|m| !m.is_empty()), code))
}

/// Replaces the registered shortcuts; reports the ones the OS refused.
fn register_hotkeys(app: &AppHandle, hotkeys: &std::collections::BTreeMap<String, Hotkey>) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let (mut map, mut errors) = (Vec::new(), Vec::new());
    for (action, h) in hotkeys {
        match shortcut_of(h) {
            Some(sc) => match gs.register(sc) {
                Ok(()) => map.push((sc, action.clone())),
                Err(e) => errors.push(format!("{action}: {e}")),
            },
            None => errors.push(format!("{action}: {} needs a modifier key", h.code)),
        }
    }
    *lock(&app.state::<RecState>().hotkeys) = map;
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

/// The screen under the mouse pointer (index into `record::screens()`).
fn display_under_cursor() -> usize {
    use device_query::DeviceQuery;
    let Some(dev) = device_query::DeviceState::checked_new() else { return 0 };
    let (x, y) = dev.get_mouse().coords;
    let (x, y) = (x as f64, y as f64);
    record::screens().iter().position(|s| x >= s.x && x < s.x + s.w && y >= s.y && y < s.y + s.h).unwrap_or(0)
}

/// Runs a global shortcut. Start and screenshot actions happen here; the rest go to the page
/// that owns that button (`hotkey-action` events), so the UI stays in step.
fn on_hotkey(app: &AppHandle, sc: &Shortcut) {
    let st = app.state::<RecState>();
    let Some(action) = lock(&st.hotkeys).iter().find(|(s, _)| s == sc).map(|(_, a)| a.clone()) else { return };
    let busy = lock(&st.rec).is_some() || app.get_webview_window("controls").is_some();
    let report = |e: String| drop(app.emit_to("main", "recording-error", e));
    match action.as_str() {
        "startStudioRecording" | "startInstantRecording" if !busy => {
            let mode = if action == "startInstantRecording" { "instant" } else { "studio" };
            let target = Target { display: display_under_cursor(), area: None, window: None, camera_only: false };
            if let Err(e) = prepare(app, &st, target, current(&st).recording_countdown, mode) {
                report(err(e));
            }
        }
        "startStudioRecording" | "startInstantRecording" => {}
        "screenshotDisplay" | "screenshotWindow" => {
            let front = (action == "screenshotWindow").then(|| record::windows().into_iter().next()).flatten();
            let target = match front {
                Some(w) => Target { display: w.display, area: None, window: Some(w.id), camera_only: false },
                None => Target { display: display_under_cursor(), area: None, window: None, camera_only: false },
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move { drop(take_screenshot(app, target).await) });
        }
        "stopRecording" | "togglePauseRecording" | "restartRecording" => drop(app.emit_to("controls", "hotkey-action", &action)),
        other => {
            if let (Some(w), "openRecordingPicker") = (app.get_webview_window("main"), other) {
                let _ = w.unminimize().and_then(|_| w.show()).and_then(|_| w.set_focus());
            }
            let _ = app.emit_to("main", "hotkey-action", other);
        }
    }
}

#[tauri::command]
fn log(msg: String) {
    eprintln!("[ui] {msg}");
}

/// Frameless, transparent, always-on-top window hidden from screen recordings.
fn overlay<'a>(app: &'a AppHandle, label: &str, page: &str, w: f64, h: f64) -> tauri::Result<WebviewWindowBuilder<'a, tauri::Wry, AppHandle>> {
    Ok(WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        // Like Cap's excluded windows: the bubble and controls never appear in the recording itself.
        .content_protected(true))
}

/// The primary screen's position and size in logical points.
fn primary(app: &AppHandle) -> tauri::Result<(f64, f64, f64, f64)> {
    Ok(app.primary_monitor()?.map_or((0.0, 0.0, 1440.0, 900.0), |m| {
        let k = m.scale_factor();
        (m.position().x as f64 / k, m.position().y as f64 / k, m.size().width as f64 / k, m.size().height as f64 / k)
    }))
}

fn camera_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window("camera") {
        return Ok(w);
    }
    let (sx, sy, sw, sh) = primary(app)?;
    // Cap's default: 230 pt preview plus a 56 pt hover toolbar, bottom-right with a 100 pt margin.
    overlay(app, "camera", "camera.html", 230.0, 286.0)?.position(sx + sw - 330.0, sy + sh - 386.0).build()
}

fn controls_window(app: &AppHandle, countdown: u32, mode: &str) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window("controls") {
        return Ok(w);
    }
    let (sx, sy, sw, sh) = primary(app)?;
    // A 40 pt bar with 12 pt of transparent padding around it (Cap: h-10 inside p-3).
    let (w, h) = (330.0, 64.0);
    overlay(app, "controls", &format!("controls.html?countdown={countdown}&mode={mode}"), w, h)?.position(sx + (sw - w) / 2.0, sy + sh - h - 40.0).build()
}

/// The Instant result card, bottom-left like Cap's recordings overlay.
fn overlay_window(app: &AppHandle, root: &Path) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window("recordings-overlay") {
        w.close()?;
    }
    let (sx, sy, _, sh) = primary(app)?;
    let path: String = root.display().to_string().bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let (w, h) = (300.0, 200.0);
    overlay(app, "recordings-overlay", &format!("overlay.html?path={path}"), w, h)?.position(sx + 20.0, sy + sh - h - 60.0).focused(false).build()
}

pub fn run() -> anyhow::Result<()> {
    tauri::Builder::default()
        .manage(RecState { mode: Mutex::new("studio".into()), ..Default::default() })
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        on_hotkey(app, sc);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            devices,
            windows,
            window_thumbnail,
            set_camera,
            set_mic,
            set_system_audio,
            camera_style,
            selection,
            camera_frame,
            open_target,
            prepare_recording,
            start_recording,
            pause_recording,
            resume_recording,
            recording_state,
            stop_recording,
            restart_recording,
            delete_recording,
            take_screenshot,
            recordings,
            screenshots,
            screenshot_thumbnail,
            open_editor,
            open_screenshot,
            open_teleprompter,
            get_settings,
            set_settings,
            pick_recordings_folder,
            default_recordings_folder,
            format_project_name,
            os,
            open_settings,
            instant_info,
            instant_thumbnail,
            copy_video,
            export_video,
            reveal,
            close_overlay,
            log
        ])
        .setup(|app| {
            let s = settings::load(&app.path().app_data_dir()?);
            *lock(&app.state::<RecState>().settings) = s.clone();
            if let Err(e) = register_hotkeys(app.handle(), &s.hotkeys) {
                eprintln!("shortcuts: {e}");
            }
            let main = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("recorder.html".into()))
                .title("Precap")
                .inner_size(330.0, 395.0)
                .resizable(false)
                .maximizable(false)
                .theme(Some(tauri::Theme::Dark));
            #[cfg(target_os = "macos")]
            let main = main.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
            main.build()?;
            Ok(())
        })
        .build(crate::editor::context())?
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let st = app.state::<RecState>();
                // Quitting mid-recording keeps the take, like Stop, instead of orphaning ffmpeg.
                if let Some(rec) = lock(&st.rec).take() {
                    let _ = rec.stop();
                }
                *lock(&st.preview) = None;
                *lock(&st.meter) = None;
                record::stop_all_children();
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_chords_become_shortcuts() {
        let hk = |code: &str, meta: bool, shift: bool| Hotkey { code: code.into(), meta, shift, ..Default::default() };
        assert_eq!(shortcut_of(&hk("KeyR", true, true)), Some(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyR)));
        assert_eq!(shortcut_of(&hk("F5", false, false)), Some(Shortcut::new(None, Code::F5)), "F-keys may stand alone");
        assert_eq!(shortcut_of(&hk("KeyR", false, false)), None, "a bare letter would hijack typing");
        assert_eq!(shortcut_of(&hk("NotAKey", true, false)), None);
    }
}
