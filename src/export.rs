//! Export: decode each timeline clip with ffmpeg, composite every frame in Rust,
//! and pipe raw RGBA into one ffmpeg encoder that also lays down the mic audio.

use crate::project::{self, BlurMode, Config, CursorLog, Meta, ZoomMode};
use crate::render::{self, Frame, Image, Rect, Sprite};
use crate::segment;
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

pub struct Project {
    pub root: PathBuf,
    pub meta: Meta,
    pub cfg: Config,
    /// One per recording segment.
    pub cursors: Vec<CursorLog>,
    /// Pixel size of each segment's screen video.
    pub display_sizes: Vec<(usize, usize)>,
    sprites: HashMap<String, Sprite>,
    arrow: Sprite,
}

impl Project {
    pub fn load(root: &Path) -> Result<Self> {
        let meta: Meta = project::read(&root.join(project::META))?;
        let cfg = project::load_config(root, &meta)?;
        let cursors = meta
            .segments
            .iter()
            .map(|s| s.cursor.as_ref().map(|p| root.join(p)).filter(|p| p.exists()).map(|p| project::read(&p)).unwrap_or(Ok(CursorLog::default())))
            .collect::<Result<_>>()?;
        // Cap stores one PNG per cursor shape; a missing or broken one falls back to the arrow.
        let sprites = meta
            .cursors
            .iter()
            .filter_map(|(id, c)| {
                let img = image::open(root.join(&c.image_path)).ok()?.to_rgba8();
                let img = Image { w: img.width() as usize, h: img.height() as usize, px: img.into_raw() };
                Some((id.clone(), render::trim(Sprite { img, hot: (c.hotspot.x, c.hotspot.y) })))
            })
            .collect();
        let display_sizes = meta.segments.iter().map(|s| probe_size(&root.join(&s.display.path))).collect::<Result<_>>()?;
        Ok(Self { root: root.to_path_buf(), meta, cfg, cursors, display_sizes, sprites, arrow: render::trim(render::arrow_sprite()) })
    }
}

/// Damped spring stepped at 1 ms: tension pulls toward the target, friction resists velocity.
#[derive(Default, Clone, Copy)]
struct Spring {
    pos: f64,
    vel: f64,
}

impl Spring {
    fn step(&mut self, target: f64, tension: f64, friction: f64, mass: f64, dt: f64) {
        let n = (dt * 1000.0).ceil().max(1.0) as usize;
        let h = dt / n as f64;
        for _ in 0..n {
            let a = (tension * (target - self.pos) - friction * self.vel) / mass;
            self.vel += a * h;
            self.pos += self.vel * h;
        }
    }
}

/// Smoothed cursor and zoom "camera", advanced once per output frame.
#[derive(Default)]
struct Motion {
    started: bool,
    cursor: [Spring; 2],
    focus: [Spring; 2],
    zoom: Spring,
}

impl Motion {
    fn advance(&mut self, cfg: &Config, dt: f64, t: f64, raw: Option<(f64, f64)>) -> (Option<(f64, f64)>, Rect) {
        let zoom = cfg.timeline.zoom_segments.iter().find(|z| t >= z.start && t < z.end);
        let zoom_target = zoom.map_or(1.0, |z| z.amount.max(1.0));
        let c = &cfg.cursor;
        let s = &cfg.screen_movement_spring;
        if !self.started {
            self.started = true;
            let (x, y) = raw.unwrap_or((0.5, 0.5));
            self.cursor = [Spring { pos: x, vel: 0.0 }, Spring { pos: y, vel: 0.0 }];
            self.focus = self.cursor;
            self.zoom.pos = zoom_target;
        }
        let cursor = raw.map(|(x, y)| {
            if c.raw {
                return (x, y);
            }
            self.cursor[0].step(x, c.tension, c.friction, c.mass, dt);
            self.cursor[1].step(y, c.tension, c.friction, c.mass, dt);
            (self.cursor[0].pos, self.cursor[1].pos)
        });
        let focus = match zoom.map(|z| &z.mode) {
            Some(ZoomMode::Manual { x, y }) => (*x, *y),
            _ => cursor.unwrap_or((0.5, 0.5)),
        };
        self.focus[0].step(focus.0, s.stiffness, s.damping, s.mass, dt);
        self.focus[1].step(focus.1, s.stiffness, s.damping, s.mass, dt);
        self.zoom.step(zoom_target, s.stiffness, s.damping, s.mass, dt);

        let size = 1.0 / self.zoom.pos.max(1.0);
        let corner = |f: f64| (f - size / 2.0).clamp(0.0, 1.0 - size);
        (cursor, Rect { x: corner(self.focus[0].pos), y: corner(self.focus[1].pos), w: size, h: size })
    }
}

fn cursor_at(log: &CursorLog, secs: f64) -> Option<(f64, f64, &str)> {
    let i = log.moves.partition_point(|m| m.time_ms <= secs * 1000.0);
    log.moves.get(i.saturating_sub(1)).map(|m| (m.x, m.y, m.cursor_id.as_str()))
}

pub fn probe_size(path: &Path) -> Result<(usize, usize)> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0:s=x"])
        .arg(path)
        .output()
        .context("running ffprobe")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (w, h) = text.trim().split_once('x').with_context(|| format!("no video stream in {}", path.display()))?;
    Ok((w.parse()?, h.parse()?))
}

/// Streams fixed-rate RGBA frames out of ffmpeg. Holds the last frame once the source runs out.
struct Decoder {
    child: Child,
    out: ChildStdout,
    img: Image,
    done: bool,
    /// Frames read so far; tells callers whether `img` changed.
    seq: u64,
}

impl Decoder {
    fn spawn(path: &Path, seek: f64, timescale: f64, fps: u32, extra_filter: &str, w: usize, h: usize) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", &format!("{:.6}", seek.max(0.0)), "-i"])
            .arg(path)
            .args(["-an", "-vf", &format!("setpts=(PTS-STARTPTS)/{timescale},fps={fps}{extra_filter}")])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to start ffmpeg")?;
        let out = child.stdout.take().context("ffmpeg stdout")?;
        Ok(Self { child, out, img: Image { w, h, px: vec![0; w * h * 4] }, done: false, seq: 0 })
    }

    fn next(&mut self) -> Result<&Image> {
        if !self.done {
            match self.out.read_exact(&mut self.img.px) {
                Ok(()) => self.seq += 1,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => self.done = true,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(&self.img)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn output_size(cfg: &Config, src_w: usize, src_h: usize, height: usize) -> (usize, usize) {
    let aspect = match cfg.aspect_ratio.as_deref() {
        Some("wide") => 16.0 / 9.0,
        Some("vertical") => 9.0 / 16.0,
        Some("square") => 1.0,
        Some("classic") => 4.0 / 3.0,
        _ => src_w as f64 / src_h as f64,
    };
    let even = |v: f64| ((v / 2.0).round() as usize * 2).max(2);
    (even(height as f64 * aspect), even(height as f64))
}

pub enum Target {
    /// Output path, x264 CRF (lower = better quality, bigger file).
    Video(PathBuf, u8),
    /// Silent looping GIF at the given path.
    Gif(PathBuf),
    /// Output time in seconds, PNG path.
    Still(f64, PathBuf),
}

/// A `.gif` path means a GIF, anything else an mp4 at the given CRF.
pub fn target(path: PathBuf, crf: u8) -> Target {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")) { Target::Gif(path) } else { Target::Video(path, crf) }
}

/// Everything fixed for one render: output size, layout, background, frames per clip.
pub struct Scene {
    pub w: usize,
    pub h: usize,
    pub fps: u32,
    pub frames: Vec<usize>,
    pub total: usize,
    layout: render::Layout,
    bg: Image,
    bg_key: String,
}

impl Scene {
    /// Reuses `prev`'s background when the source and size are unchanged (image decode is slow).
    pub fn new(p: &Project, fps: u32, height: usize, prev: Option<Scene>) -> Result<Self> {
        let (src_w, src_h) = *p.display_sizes.first().context("project has no segments")?;
        let (w, h) = output_size(&p.cfg, src_w, src_h, height);
        let has_camera = p.meta.segments.iter().any(|s| s.camera.is_some());
        let layout = render::layout(&p.cfg, w, h, src_w as f64 / src_h as f64, has_camera);
        let bg_key = format!("{w}x{h} {}", serde_json::to_string(&p.cfg.background.source)?);
        let bg = match prev {
            Some(prev) if prev.bg_key == bg_key => prev.bg,
            _ => render::background(&p.cfg, w, h)?,
        };
        let clips = &p.cfg.timeline.segments;
        let frames: Vec<usize> = clips.iter().map(|c| ((c.end - c.start) / c.timescale * fps as f64).round() as usize).collect();
        let total = frames.iter().sum();
        if total == 0 {
            bail!("timeline is empty");
        }
        Ok(Self { w, h, fps, frames, total, layout, bg, bg_key })
    }

    pub fn has_camera_layer(&self) -> bool {
        self.layout.cam.is_some()
    }

    pub fn dt(&self) -> f64 {
        1.0 / self.fps as f64
    }

    /// Which clip a timeline frame falls in, and the frame index inside that clip.
    pub fn locate(&self, frame_no: usize) -> (usize, usize) {
        let mut rest = frame_no.min(self.total - 1);
        for (k, &n) in self.frames.iter().enumerate() {
            if rest < n {
                return (k, rest);
            }
            rest -= n;
        }
        unreachable!("frame_no is clamped to total")
    }

    /// Screen (+camera) decoders for clip `k`, starting at its frame `i`.
    /// `max_w` shrinks the screen inside ffmpeg (live preview); None keeps full resolution.
    pub fn open(&self, p: &Project, k: usize, i: usize, max_w: Option<usize>) -> Result<Decoders> {
        let clip = &p.cfg.timeline.segments[k];
        let seg = p.meta.segments.get(clip.recording_segment).context("timeline points at a missing segment")?;
        let at = clip.start + i as f64 * self.dt() * clip.timescale;
        let (sw, sh) = p.display_sizes[clip.recording_segment];
        let (dw, dh) = match max_w {
            Some(m) if m < sw => (m, (sh * m / sw) & !1),
            _ => (sw, sh),
        };
        let scale = if (dw, dh) == (sw, sh) { String::new() } else { format!(",scale={dw}:{dh}") };
        let display = Decoder::spawn(&p.root.join(&seg.display.path), at, clip.timescale, self.fps, &scale, dw, dh)?;
        let camera = match (&seg.camera, self.layout.cam) {
            (Some(c), Some(cam)) => {
                let side = if max_w.is_some() { 360 } else { cam.w.round().max(2.0) as usize };
                let filter = format!(",crop='min(iw,ih)':'min(iw,ih)',scale={side}:{side}");
                let seek = at + seg.display.start_time - c.start_time;
                Some(Decoder::spawn(&p.root.join(&c.path), seek, clip.timescale, self.fps, &filter, side, side)?)
            }
            _ => None,
        };
        Ok(Decoders { display, camera, mask: None })
    }
}

pub struct Decoders {
    display: Decoder,
    camera: Option<Decoder>,
    /// Person matte of camera frame `seq`; None inside means segmentation failed.
    mask: Option<(u64, Option<Image>)>,
}

impl Decoders {
    /// Reads the next frame of every stream.
    pub fn advance(&mut self) -> Result<()> {
        self.display.next()?;
        if let Some(c) = &mut self.camera {
            c.next()?;
        }
        Ok(())
    }

    /// Composites the most recently read frames into `out`.
    pub fn draw(&mut self, p: &Project, scene: &Scene, s: &FrameState, out: &mut [u8]) {
        let mode = p.cfg.camera.background_blur.mode;
        let mut cutout = None;
        if let (Some(cam), true) = (&self.camera, mode != BlurMode::Off && cfg!(target_os = "macos")) {
            // Vision runs once per decoded camera frame, however often it is redrawn.
            if self.mask.as_ref().is_none_or(|m| m.0 != cam.seq) {
                self.mask = Some((cam.seq, segment::person_mask(&cam.img).map_err(|e| eprintln!("camera background: {e:#}")).ok()));
            }
            cutout = self.mask.as_ref().and_then(|m| m.1.as_ref()).map(|m| segment::apply(&cam.img, m, mode));
        }
        let sprite = s.cursor_id.as_deref().map(|id| p.sprites.get(id).unwrap_or(&p.arrow));
        let frame = Frame {
            screen: &self.display.img,
            camera: cutout.as_ref().or(self.camera.as_ref().map(|c| &c.img)),
            crop: s.crop,
            cursor: if p.cfg.cursor.hide { None } else { s.cursor.zip(sprite) },
        };
        render::compose(out, &scene.bg, &scene.layout, &frame);
    }
}

#[derive(Clone)]
pub struct FrameState {
    cursor: Option<(f64, f64)>,
    cursor_id: Option<String>,
    crop: Rect,
}

/// Spring state replayed frame by frame from the start, so a preview of any
/// frame matches the export exactly. Moving forward continues where it left off.
#[derive(Default)]
pub struct Playhead {
    motion: Motion,
    next: usize,
    last: Option<FrameState>,
}

impl Playhead {
    pub fn advance_to(&mut self, p: &Project, scene: &Scene, frame_no: usize) -> FrameState {
        if frame_no + 1 < self.next || self.last.is_none() {
            *self = Self::default();
        }
        while self.next <= frame_no {
            let (k, i) = scene.locate(self.next);
            let clip = &p.cfg.timeline.segments[k];
            let seg = &p.meta.segments[clip.recording_segment];
            let src_t = clip.start + i as f64 * scene.dt() * clip.timescale;
            let raw = cursor_at(&p.cursors[clip.recording_segment], src_t + seg.display.start_time);
            let (cursor, crop) = self.motion.advance(&p.cfg, scene.dt(), self.next as f64 * scene.dt(), raw.map(|(x, y, _)| (x, y)));
            self.last = Some(FrameState { cursor, cursor_id: raw.map(|r| r.2.to_string()), crop });
            self.next += 1;
        }
        self.last.clone().expect("advanced at least once")
    }
}

pub fn render(p: &Project, fps: u32, height: usize, target: Target, progress: &mut dyn FnMut(usize, usize)) -> Result<()> {
    let scene = Scene::new(p, fps, height, None)?;
    let (w, h, total) = (scene.w, scene.h, scene.total);
    let mut out = vec![0u8; w * h * 4];
    let mut playhead = Playhead::default();
    let mut enc = match target {
        Target::Still(t, png) => {
            let frame_no = ((t * fps as f64) as usize).min(total - 1);
            let state = playhead.advance_to(p, &scene, frame_no);
            let (k, i) = scene.locate(frame_no);
            let mut dec = scene.open(p, k, i, None)?;
            dec.advance()?;
            dec.draw(p, &scene, &state, &mut out);
            image::save_buffer(&png, &out, w as u32, h as u32, image::ColorType::Rgba8)?;
            println!("wrote {} ({w}x{h}, frame {frame_no} of {total})", png.display());
            return Ok(());
        }
        Target::Video(path, crf) => spawn_encoder(p, &path, &scene, crf)?,
        Target::Gif(path) => spawn_gif_encoder(&path, &scene)?,
    };
    let stdin = enc.stdin.as_mut().context("encoder stdin")?;
    let mut frame_no = 0;
    for (k, &n) in scene.frames.iter().enumerate() {
        let mut dec = scene.open(p, k, 0, None)?;
        for _ in 0..n {
            let state = playhead.advance_to(p, &scene, frame_no);
            dec.advance()?;
            dec.draw(p, &scene, &state, &mut out);
            stdin.write_all(&out).context("encoder stopped; see ffmpeg output above")?;
            frame_no += 1;
            progress(frame_no, total);
        }
    }
    drop(enc.stdin.take());
    if !enc.wait()?.success() {
        bail!("ffmpeg encoder failed");
    }
    Ok(())
}

/// Adds one ffmpeg input per clip holding that clip's slice of `track` (or silence), numbered
/// from `*next`, and returns the graph for them ending in `[{label}]`: each slice delayed by the
/// track's start offset, sped up with its clip, then all concatenated.
fn add_track(cmd: &mut Command, p: &Project, scene: &Scene, next: &mut usize, label: &str, track: fn(&project::Segment) -> Option<&project::Track>) -> String {
    let mut graph = String::new();
    for (k, (clip, &n)) in p.cfg.timeline.segments.iter().zip(&scene.frames).enumerate() {
        let out_len = n as f64 * scene.dt();
        let seg = &p.meta.segments[clip.recording_segment];
        let delay = match track(seg) {
            Some(m) => {
                let seek = clip.start + seg.display.start_time - m.start_time;
                cmd.args(["-ss", &format!("{:.6}", seek.max(0.0)), "-t", &format!("{:.6}", clip.end - clip.start), "-i"]);
                cmd.arg(p.root.join(&m.path));
                (-seek).max(0.0)
            }
            None => {
                cmd.args(["-f", "lavfi", "-t", &format!("{out_len:.6}"), "-i", "anullsrc=r=48000:cl=stereo"]);
                0.0
            }
        };
        graph += &format!(
            "[{}:a]aresample=48000,aformat=channel_layouts=stereo,adelay={:.0}:all=1,{},apad,atrim=duration={out_len:.6}[{label}{k}];",
            *next,
            delay * 1000.0,
            atempo(clip.timescale)
        );
        *next += 1;
    }
    let n = scene.frames.len();
    graph + &(0..n).map(|k| format!("[{label}{k}]")).collect::<String>() + &format!("concat=n={n}:v=0:a=1[{label}];")
}

/// Cap's volume sliders: dB gain, with -30 dB or lower meaning silence.
fn gain(db: f64) -> String {
    if db <= -30.0 { "volume=0".into() } else { format!("volume={db}dB") }
}

/// Adds the timeline's audio as ffmpeg inputs numbered from `first_input` and returns a
/// filter graph ending in `[a]`: the mic (optionally cleaned up) mixed with system audio.
fn add_audio(cmd: &mut Command, p: &Project, scene: &Scene, first_input: usize) -> String {
    let (a, mut next) = (&p.cfg.audio, first_input);
    // loudnorm upsamples internally, so resample back to 48 kHz.
    let improve = if a.improve { ",afftdn=nf=-25,loudnorm=I=-16:TP=-1.5:LRA=11,aresample=48000" } else { "" };
    let mut graph = add_track(cmd, p, scene, &mut next, "m", |s| s.mic.as_ref()) + &format!("[m]{}{improve}", gain(a.mic_volume_db));
    if p.meta.segments.iter().any(|s| s.system_audio.is_some()) {
        graph += "[mic];";
        graph += &add_track(cmd, p, scene, &mut next, "s", |s| s.system_audio.as_ref());
        graph += &format!("[s]{}[sys];[mic][sys]amix=inputs=2:duration=first:normalize=0", gain(a.system_volume_db));
    }
    graph + if a.mute { ",volume=0[a]" } else { "[a]" }
}

/// ffmpeg's atempo only accepts 0.5x-100x, so slower speeds chain several.
fn atempo(mut speed: f64) -> String {
    let mut parts = Vec::new();
    while speed < 0.5 {
        parts.push("atempo=0.5".to_string());
        speed /= 0.5;
    }
    parts.push(format!("atempo={speed}"));
    parts.join(",")
}

/// One ffmpeg process: raw RGBA video on stdin plus the timeline's audio.
fn spawn_encoder(p: &Project, out: &Path, scene: &Scene, crf: u8) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    let size = format!("{}x{}", scene.w, scene.h);
    cmd.args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-r", &scene.fps.to_string(), "-i", "-"]);
    let graph = format!("[0:v]scale=out_color_matrix=bt709,format=yuv420p[v];{}", add_audio(&mut cmd, p, scene, 1));
    cmd.args(["-filter_complex", &graph, "-map", "[v]", "-map", "[a]"])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", &crf.to_string()])
        .args(["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709"])
        .args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
        .arg(out)
        .stdin(Stdio::piped());
    cmd.spawn().context("failed to start ffmpeg encoder")
}

/// Raw RGBA on stdin to a looping GIF. A palette per frame (`stats_mode=single` + `new=1`) streams
/// in flat memory; a global palette would buffer every frame until the end.
fn spawn_gif_encoder(out: &Path, scene: &Scene) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    let size = format!("{}x{}", scene.w, scene.h);
    cmd.args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-r", &scene.fps.to_string(), "-i", "-"])
        .args(["-filter_complex", "[0:v]split[a][b];[a]palettegen=stats_mode=single[p];[b][p]paletteuse=new=1", "-loop", "0"])
        .arg(out)
        .stdin(Stdio::piped());
    cmd.spawn().context("failed to start ffmpeg encoder")
}

/// Renders just the timeline's audio (for the editor's playback).
pub fn render_audio(p: &Project, scene: &Scene, out: &Path) -> Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-y"]);
    let graph = add_audio(&mut cmd, p, scene, 0);
    let status = cmd.args(["-filter_complex", &graph, "-map", "[a]", "-c:a", "aac", "-b:a", "128k"]).arg(out).status()?;
    if !status.success() {
        bail!("ffmpeg could not render the audio preview");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic 2 s project exports to a GIF of the requested size and the timeline's length.
    #[test]
    fn exports_gif() {
        let root = std::env::temp_dir().join(format!("gif export {}", std::process::id()));
        let seg = root.join("content/segments/segment-0");
        std::fs::create_dir_all(&seg).unwrap();
        let ff = Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=320x240:r=30:d=2", "-c:v", "libx264", "-pix_fmt", "yuv420p"]).arg(seg.join("display.mkv")).status().unwrap();
        assert!(ff.success());
        let meta = Meta { segments: vec![project::Segment { display: project::Track { path: "content/segments/segment-0/display.mkv".into(), fps: None, start_time: 0.0 }, camera: None, mic: None, system_audio: None, cursor: None }], ..Default::default() };
        project::write(&root.join(project::META), &meta).unwrap();
        let p = Project::load(&root).unwrap();
        let out = root.join("out.gif");
        let (w, h) = (Scene::new(&p, 15, 120, None).unwrap().w, 120);
        render(&p, 15, 120, target(out.clone(), 20), &mut |_, _| {}).unwrap();
        let probe = |entry: &str| String::from_utf8(Command::new("ffprobe").args(["-v", "error", "-show_entries", entry, "-of", "csv=p=0"]).arg(&out).output().unwrap().stdout).unwrap().trim().to_string();
        assert_eq!(probe("stream=codec_name,width,height"), format!("gif,{w},{h}"));
        let secs: f64 = probe("format=duration").parse().unwrap();
        assert!((secs - 2.0).abs() < 0.15, "duration {secs}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn atempo_chains_below_half_speed() {
        assert_eq!(atempo(0.25), "atempo=0.5,atempo=0.5");
        assert_eq!(atempo(8.0), "atempo=8");
    }

    #[test]
    fn spring_settles_on_target() {
        let mut s = Spring::default();
        for _ in 0..300 {
            s.step(1.0, 470.0, 70.0, 3.0, 1.0 / 60.0);
        }
        assert!((s.pos - 1.0).abs() < 1e-3 && s.vel.abs() < 1e-3);
    }

    #[test]
    fn zoom_crop_stays_inside_frame() {
        let mut cfg = Config::default();
        cfg.timeline.zoom_segments.push(project::ZoomSegment { start: 0.0, end: 10.0, amount: 2.0, mode: ZoomMode::Auto });
        let mut m = Motion::default();
        let mut crop = Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
        for i in 0..120 {
            crop = m.advance(&cfg, 1.0 / 30.0, i as f64 / 30.0, Some((0.99, 0.01))).1;
        }
        assert!((crop.w - 0.5).abs() < 1e-3, "zoomed to 2x");
        assert!(crop.x + crop.w <= 1.0 && crop.y >= 0.0, "pinned to the corner, not past it");
    }
}
