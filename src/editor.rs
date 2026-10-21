//! The GUI editor: a Tauri window whose HTML/JS front end (ui/) edits the
//! project config and asks Rust for preview frames, audio and exports.
//! Preview frames come from the same renderer as the export, so what you see is what you get.

use crate::export::{self, Decoders, Playhead, Project, Scene};
use crate::project::{self, Config};
use crate::autozoom;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{Emitter, State, WebviewUrl, WebviewWindowBuilder};

const FPS: u32 = 30;
/// Screen frames are shrunk to this width inside ffmpeg for preview; plenty for a window.
const PREVIEW_MAX_W: usize = 1920;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

struct Open {
    /// recording segment, clip start, clip speed, camera layer on
    key: (usize, u64, u64, bool),
    /// Clip frame index currently decoded.
    have: usize,
    dec: Decoders,
}

struct Preview {
    p: Project,
    applied: String,
    height: usize,
    scene: Option<Scene>,
    playhead: Playhead,
    open: Option<Open>,
    out: Vec<u8>,
}

impl Preview {
    fn frame(&mut self, t: f64, cfg: Value, height: usize) -> anyhow::Result<Vec<u8>> {
        let applied = cfg.to_string();
        if applied != self.applied || height != self.height || self.scene.is_none() {
            self.p.cfg = serde_json::from_value(cfg)?;
            self.scene = Some(Scene::new(&self.p, FPS, height, self.scene.take())?);
            self.playhead = Playhead::default();
            (self.applied, self.height) = (applied, height);
        }
        let scene = self.scene.as_ref().expect("built above");
        let frame_no = ((t * FPS as f64).round() as usize).min(scene.total - 1);
        let (k, i) = scene.locate(frame_no);
        let clip = &self.p.cfg.timeline.segments[k];
        let key = (clip.recording_segment, clip.start.to_bits(), clip.timescale.to_bits(), scene.has_camera_layer());

        // Reuse the open decoder when possible: same frame (slider drag) or a short hop forward (playback).
        match &mut self.open {
            Some(o) if o.key == key && o.have == i => {}
            Some(o) if o.key == key && i > o.have && i <= o.have + 45 => {
                for _ in o.have..i {
                    o.dec.advance()?;
                }
                o.have = i;
            }
            _ => {
                let mut dec = scene.open(&self.p, k, i, Some(PREVIEW_MAX_W))?;
                dec.advance()?;
                self.open = Some(Open { key, have: i, dec });
            }
        }
        let state = self.playhead.advance_to(&self.p, scene, frame_no);
        self.out.resize(scene.w * scene.h * 4, 0);
        let o = self.open.as_mut().expect("opened above");
        o.dec.draw(&self.p, scene, &state, &mut self.out);

        let rgb: Vec<u8> = self.out.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode(&rgb, scene.w as u32, scene.h as u32, image::ExtendedColorType::Rgb8)?;
        Ok(jpeg)
    }
}

struct Editor {
    root: PathBuf,
    preview: Mutex<Preview>,
}

#[derive(Serialize)]
struct SegmentInfo {
    duration: f64,
    has_camera: bool,
    has_mic: bool,
    has_system_audio: bool,
}

#[derive(Serialize)]
struct ProjectInfo {
    name: String,
    path: String,
    fps: u32,
    source_width: usize,
    source_height: usize,
    config: Value,
    segments: Vec<SegmentInfo>,
    /// The "Default zoom amount" setting, for zooms dragged out on the timeline.
    #[serde(rename = "defaultZoomAmount")]
    default_zoom_amount: f64,
    /// Whether this OS can separate the person from the camera background.
    #[serde(rename = "canSegment")]
    can_segment: bool,
}

#[tauri::command]
fn project(app: tauri::AppHandle, ed: State<Editor>) -> CmdResult<ProjectInfo> {
    let pv = ed.preview.lock().map_err(err)?;
    let p = &pv.p;
    let segments = p
        .meta
        .segments
        .iter()
        .map(|s| {
            let duration = crate::record::probe(&p.root.join(&s.display.path), "duration").unwrap_or(0.0);
            SegmentInfo { duration, has_camera: s.camera.is_some(), has_mic: s.mic.is_some(), has_system_audio: s.system_audio.is_some() }
        })
        .collect();
    let (source_width, source_height) = p.display_sizes[0];
    let name = if p.meta.pretty_name.is_empty() { file_stem(&ed.root) } else { p.meta.pretty_name.clone() };
    Ok(ProjectInfo { name, path: ed.root.display().to_string(), fps: FPS, source_width, source_height, config: serde_json::to_value(&p.cfg).map_err(err)?, segments, default_zoom_amount: settings(&app).default_zoom_amount, can_segment: cfg!(target_os = "macos") })
}

/// JPEG of the composited frame at `t` seconds on the output timeline.
#[tauri::command]
async fn frame(ed: State<'_, Editor>, t: f64, config: Value, height: usize) -> CmdResult<Response> {
    let jpeg = ed.preview.lock().map_err(err)?.frame(t, config, height.clamp(90, 2160)).map_err(err)?;
    Ok(Response::new(jpeg))
}

/// Deep-merges our edits into the file on disk, so fields this app doesn't know
/// (Cap writes many) survive a save.
pub(crate) fn merge(base: &mut Value, edits: Value) {
    match (base, edits) {
        // A tagged object whose "type" changed (e.g. wallpaper -> color) is replaced, not mixed.
        (Value::Object(b), Value::Object(e)) if b.get("type").is_none_or(|t| Some(t) == e.get("type")) => {
            for (k, v) in e {
                merge(b.entry(k).or_insert(Value::Null), v);
            }
        }
        (b, e) => *b = e,
    }
}

#[tauri::command]
fn save(ed: State<Editor>, config: Value) -> CmdResult<()> {
    let _: Config = serde_json::from_value(config.clone()).map_err(err)?;
    let path = ed.root.join(project::CONFIG);
    let mut on_disk: Value = if path.exists() { project::read(&path).map_err(err)? } else { Value::Object(Default::default()) };
    merge(&mut on_disk, config);
    project::write(&path, &on_disk).map_err(err)
}

#[tauri::command]
fn rename(ed: State<Editor>, name: String) -> CmdResult<()> {
    let path = ed.root.join(project::META);
    let mut meta: Value = project::read(&path).map_err(err)?;
    meta["pretty_name"] = Value::String(name.clone());
    project::write(&path, &meta).map_err(err)?;
    ed.preview.lock().map_err(err)?.p.meta.pretty_name = name;
    Ok(())
}

/// The timeline's mixed audio as m4a bytes, for playback in the editor.
#[tauri::command]
async fn audio(ed: State<'_, Editor>, config: Value) -> CmdResult<Response> {
    let mut p = Project::load(&ed.root).map_err(err)?;
    p.cfg = serde_json::from_value(config).map_err(err)?;
    let scene = Scene::new(&p, FPS, 90, None).map_err(err)?;
    // Unique per call: quick edits can overlap two renders.
    let tmp = crate::os::temp_file("m4a").map_err(err)?;
    export::render_audio(&p, &scene, &tmp).map_err(err)?;
    let bytes = std::fs::read(&tmp).map_err(err)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(Response::new(bytes))
}

#[derive(Serialize)]
struct Waveform {
    /// mic time = display time + offset
    offset: f64,
    step: f64,
    peaks: Vec<f32>,
}

/// Peak level every 50 ms of a segment's mic, for drawing in the clip.
#[tauri::command]
async fn waveform(ed: State<'_, Editor>, segment: usize) -> CmdResult<Option<Waveform>> {
    let (path, offset) = {
        let pv = ed.preview.lock().map_err(err)?;
        let Some(seg) = pv.p.meta.segments.get(segment) else { return Ok(None) };
        // shortcut: one waveform per clip (the mic, else system audio); Cap overlays both.
        let Some(mic) = seg.mic.as_ref().or(seg.system_audio.as_ref()) else { return Ok(None) };
        (pv.p.root.join(&mic.path), seg.display.start_time - mic.start_time)
    };
    let out = Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(&path).args(["-ac", "1", "-ar", "4000", "-f", "s16le", "-"]).output().map_err(err)?;
    let peaks = out
        .stdout
        .chunks(400) // 200 samples = 50 ms
        .map(|c| c.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]).unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0)
        .collect();
    Ok(Some(Waveform { offset, step: 0.05, peaks }))
}

#[tauri::command]
fn generate_zooms(app: tauri::AppHandle, ed: State<Editor>, config: Value) -> CmdResult<Value> {
    let pv = ed.preview.lock().map_err(err)?;
    let cfg: Config = serde_json::from_value(config).map_err(err)?;
    let duration = cfg.timeline.segments.iter().map(|c| (c.end - c.start) / c.timescale).sum();
    let clicks = autozoom::click_times(&pv.p.meta, &cfg, &pv.p.cursors);
    serde_json::to_value(autozoom::zoom_segments_from_clicks(&clicks, duration, settings(&app).default_zoom_amount)).map_err(err)
}

/// Copies a picked background image into the project's assets folder; returns its path.
#[tauri::command]
fn save_asset(ed: State<Editor>, request: Request) -> CmdResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else { return Err("expected raw image bytes".into()) };
    let name = request.headers().get("name").and_then(|v| v.to_str().ok()).unwrap_or("background.png");
    let name = Path::new(name).file_name().ok_or("bad file name")?;
    let dir = ed.root.join("assets");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(name);
    std::fs::write(&path, bytes).map_err(err)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn default_export_path(ed: State<Editor>) -> String {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    let dir = [if cfg!(target_os = "macos") { "Movies" } else { "Videos" }, ""].iter().map(|d| Path::new(&home).join(d)).find(|d| d.is_dir()).unwrap_or_else(|| PathBuf::from(&home));
    let name = ed.preview.lock().map(|p| p.p.meta.pretty_name.clone()).unwrap_or_default();
    let name = if name.trim().is_empty() { file_stem(&ed.root) } else { crate::os::file_name(&name) };
    dir.join(format!("{name}.mp4")).display().to_string()
}

/// Renders an mp4 (a gif for a .gif path), emitting `export-progress` events as `[done, total]`.
#[tauri::command]
async fn export(app: tauri::AppHandle, ed: State<'_, Editor>, config: Value, fps: u32, height: usize, crf: u8, path: String) -> CmdResult<String> {
    let mut p = Project::load(&ed.root).map_err(err)?;
    p.cfg = serde_json::from_value(config).map_err(err)?;
    let mut last = 0;
    export::render(&p, fps, height, export::target(PathBuf::from(&path), crf), &mut |done, total| {
        if done == total || done - last >= fps as usize / 2 {
            last = done;
            let _ = app.emit("export-progress", (done, total));
        }
    })
    .map_err(err)?;
    Ok(path)
}

/// Lets the web UI report errors to the terminal (release webviews have no devtools).
#[tauri::command]
fn log(msg: String) {
    eprintln!("[ui] {msg}");
}

/// Shows a file in Finder / Explorer / the Linux file manager.
#[tauri::command]
fn reveal(path: String) -> CmdResult<()> {
    crate::os::reveal(Path::new(&path)).map_err(err)
}

/// The recorder's settings (same app data folder: the editor is the same app).
fn settings(app: &tauri::AppHandle) -> crate::settings::Settings {
    use tauri::Manager;
    app.path().app_data_dir().map(|d| crate::settings::load(&d)).unwrap_or_default()
}

fn file_stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Shared by the editor and recorder apps (both live in this one binary).
pub fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn run(root: &Path) -> anyhow::Result<()> {
    let p = Project::load(root)?;
    let preview = Preview { p, applied: String::new(), height: 0, scene: None, playhead: Playhead::default(), open: None, out: Vec::new() };
    tauri::Builder::default()
        .manage(Editor { root: root.to_path_buf(), preview: Mutex::new(preview) })
        .invoke_handler(tauri::generate_handler![project, frame, save, rename, audio, waveform, generate_zooms, save_asset, default_export_path, export, reveal, log])
        .setup(|app| {
            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Precap")
                .inner_size(1275.0, 800.0)
                .min_inner_size(960.0, 640.0)
                .theme(Some(tauri::Theme::Dark));
            // Like Cap: content runs under a transparent title bar with the traffic lights on top.
            #[cfg(target_os = "macos")]
            let builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(tauri::LogicalPosition::new(20.0, 27.5));
            builder.build()?;
            Ok(())
        })
        .run(context())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::merge;
    use serde_json::json;

    #[test]
    fn merge_keeps_unknown_fields() {
        let mut disk = json!({"background": {"padding": 0, "frame": {"kind": "mac"}}, "captions": {"x": 1}});
        merge(&mut disk, json!({"background": {"padding": 12}, "timeline": {"segments": []}}));
        assert_eq!(disk, json!({"background": {"padding": 12, "frame": {"kind": "mac"}}, "captions": {"x": 1}, "timeline": {"segments": []}}));

        let mut src = json!({"source": {"type": "wallpaper", "path": "/a.jpg"}});
        merge(&mut src, json!({"source": {"type": "color", "value": [1, 2, 3]}}));
        assert_eq!(src, json!({"source": {"type": "color", "value": [1, 2, 3]}}));
    }
}
