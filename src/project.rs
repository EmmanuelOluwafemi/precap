//! On-disk project format. Field names mirror Cap's `recording-meta.json`,
//! `cursor.json` and `project-config.json`, so this tool can also export
//! studio recordings made by Cap itself.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::path::Path;

pub const META: &str = "recording-meta.json";
pub const CONFIG: &str = "project-config.json";

pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    // Write-then-rename so a crash never leaves a half-written project file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------- recording-meta.json ----------

#[derive(Serialize, Deserialize, Default)]
pub struct Meta {
    #[serde(default)]
    pub pretty_name: String,
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub cursors: HashMap<String, CursorImage>,
}

/// One pause-free stretch of recording. Every pause/resume starts a new one.
#[derive(Serialize, Deserialize, Clone)]
pub struct Segment {
    pub display: Track,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic: Option<Track>,
    /// What the computer played (Cap's key name too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_audio: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Track {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    /// Seconds from the segment start to this stream's first sample. The timeline
    /// runs on display-file time (as Cap's exports do); other tracks are shifted by
    /// their start_time difference from the display.
    #[serde(default)]
    pub start_time: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorImage {
    pub image_path: String,
    pub hotspot: XY,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, PartialEq)]
pub struct XY {
    pub x: f64,
    pub y: f64,
}

// ---------- cursor.json ----------

#[derive(Serialize, Deserialize, Default)]
pub struct CursorLog {
    pub moves: Vec<Move>,
    pub clicks: Vec<Click>,
}

/// `x`/`y` are normalized to the captured display (0..1, may go outside).
#[derive(Serialize, Deserialize, Clone)]
pub struct Move {
    pub cursor_id: String,
    pub time_ms: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Click {
    pub cursor_id: String,
    pub time_ms: f64,
    pub down: bool,
}

// ---------- project-config.json (the non-destructive edit) ----------
// Only the subset this tool renders; unknown Cap fields are ignored on read.

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// null = source aspect, or "wide" | "vertical" | "square" | "classic".
    pub aspect_ratio: Option<String>,
    pub background: Background,
    pub camera: Camera,
    pub cursor: Cursor,
    pub audio: Audio,
    /// Drives zoom in/out and camera pan, like Cap's field of the same name.
    pub screen_movement_spring: Spring,
    pub timeline: Timeline,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Background {
    pub source: BgSource,
    /// Percent of the shorter output side left empty around the screen.
    pub padding: f64,
    /// Percent of half the screen card's shorter side.
    pub rounding: f64,
    /// "squircle" (superellipse, n=4) or "rounded" (circular corners).
    pub rounding_type: String,
    /// 0-100 master strength; advancedShadow values are 0-100 too.
    pub shadow: f64,
    pub advanced_shadow: Shadow,
}

impl Default for Background {
    fn default() -> Self {
        Self {
            source: BgSource::Gradient { from: [71, 59, 214], to: [233, 119, 163], angle: 135.0 },
            padding: 8.0,
            rounding: 8.0,
            rounding_type: "squircle".into(),
            shadow: 60.0,
            advanced_shadow: Shadow::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BgSource {
    Wallpaper { path: Option<String> },
    Image { path: Option<String> },
    Color { value: [u8; 3] },
    Gradient {
        #[serde(default)]
        from: [u8; 3],
        #[serde(default)]
        to: [u8; 3],
        #[serde(default)]
        angle: f64,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Shadow {
    pub size: f64,
    pub opacity: f64,
    pub blur: f64,
}

impl Default for Shadow {
    fn default() -> Self {
        Self { size: 50.0, opacity: 18.0, blur: 50.0 }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Camera {
    pub hide: bool,
    pub mirror: bool,
    pub position: CameraPosition,
    /// Bubble centre, normalized to the output; overrides `position` when set.
    pub manual_position: Option<XY>,
    /// Bubble side as a percent of the shorter output side.
    pub size: f64,
    /// 100 = circle.
    pub rounding: f64,
    pub rounding_type: String,
    pub background_blur: BackgroundBlur,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
pub struct BackgroundBlur {
    pub mode: BlurMode,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum BlurMode {
    #[default]
    Off,
    Light,
    Heavy,
    Remove,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            hide: false,
            mirror: false,
            position: CameraPosition { x: "right".into(), y: "bottom".into() },
            manual_position: None,
            size: 25.0,
            rounding: 100.0,
            rounding_type: "squircle".into(),
            background_blur: BackgroundBlur::default(),
        }
    }
}

/// x: "left" | "center" | "right", y: "top" | "bottom".
#[derive(Serialize, Deserialize, Clone)]
pub struct CameraPosition {
    pub x: String,
    pub y: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Audio {
    pub mute: bool,
    /// Mic gain in dB (Cap's editor offers -30 to +10).
    pub mic_volume_db: f64,
    /// Denoise + loudness-normalize the mic (Cap's exports come out ~25 dB louder than the raw mic).
    pub improve: bool,
    /// System audio gain in dB (same -30 to +10 range; -30 or lower = muted).
    pub system_volume_db: f64,
}

impl Default for Audio {
    fn default() -> Self {
        Self { mute: false, mic_volume_db: 0.0, improve: true, system_volume_db: 0.0 }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Cursor {
    pub hide: bool,
    /// Percent; 100 = default size.
    pub size: f64,
    /// true = draw the cursor exactly where it was, no spring smoothing.
    pub raw: bool,
    pub tension: f64,
    pub mass: f64,
    pub friction: f64,
}

impl Default for Cursor {
    fn default() -> Self {
        // Cap's "mellow" preset values, read from its saved projects.
        Self { hide: false, size: 100.0, raw: false, tension: 470.0, mass: 3.0, friction: 70.0 }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Spring {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}

impl Default for Spring {
    fn default() -> Self {
        // Cap's saved default screenMovementSpring.
        Self { stiffness: 200.0, damping: 40.0, mass: 2.25 }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Timeline {
    /// Cuts and speed changes: which slice of which recording segment plays, in order.
    pub segments: Vec<Clip>,
    pub zoom_segments: Vec<ZoomSegment>,
}

/// `start`/`end` are seconds into the segment's display file; `timescale` 2.0 = double speed.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub recording_segment: usize,
    #[serde(default = "one")]
    pub timescale: f64,
    pub start: f64,
    pub end: f64,
}

fn one() -> f64 {
    1.0
}

/// `start`/`end` are seconds on the output timeline.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ZoomSegment {
    pub start: f64,
    pub end: f64,
    pub amount: f64,
    pub mode: ZoomMode,
}

/// "auto" follows the cursor; {"manual": {"x": .., "y": ..}} holds a fixed point (0..1).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ZoomMode {
    Auto,
    Manual { x: f64, y: f64 },
}

/// Loads the edit config, filling the timeline with every full segment when empty.
pub fn load_config(root: &Path, meta: &Meta) -> Result<Config> {
    let path = root.join(CONFIG);
    let mut cfg: Config = if path.exists() { read(&path)? } else { Config::default() };
    if cfg.timeline.segments.is_empty() {
        for (i, seg) in meta.segments.iter().enumerate() {
            let display = &seg.display;
            let duration = crate::record::probe(&root.join(&display.path), "duration")?;
            cfg.timeline.segments.push(Clip {
                recording_segment: i,
                timescale: 1.0,
                start: 0.0,
                end: duration,
            });
        }
    }
    Ok(cfg)
}
