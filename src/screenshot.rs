//! Screenshot mode: a still saved as a Cap-style project (`<name>.cap/original.png` +
//! `recording-meta.json` + `project-config.json`), and the screenshot editor window
//! (`precap screenshot <dir>`, page ui/screenshot.html).
//!
//! Rust renders the frame (background, shadow, rounded image, border) with Cap's
//! screenshot frame math; annotations are drawn by the page on top, as Cap does.

use crate::project::{self, BgSource, Config};
use crate::render::{self, Image, Rect};
use anyhow::{Context, Result};
use image::ImageEncoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{State, WebviewUrl, WebviewWindowBuilder};

pub const ORIGINAL: &str = "original.png";

/// Cap's placeholder config for a screenshot: transparent background, some padding and rounding.
fn default_config() -> Value {
    json!({
        "aspectRatio": null,
        "background": {
            "source": {"type": "color", "value": [255, 255, 255], "alpha": 0},
            "blur": 0, "padding": 20, "rounding": 10, "roundingType": "squircle",
            "shadow": 0, "advancedShadow": null, "crop": null, "border": null
        },
        "annotations": []
    })
}

/// Creates `<dir>/<name>.cap` (made unique), lets `capture` write `original.png` into it, and
/// adds the meta and config files. Returns the project folder.
pub fn create(dir: &Path, name: &str, capture: impl FnOnce(&Path) -> Result<()>) -> Result<PathBuf> {
    let root = crate::os::unique(dir, name, "cap");
    std::fs::create_dir_all(&root).context("creating the screenshot folder")?;
    if let Err(e) = capture(&root.join(ORIGINAL)) {
        let _ = std::fs::remove_dir_all(&root);
        return Err(e);
    }
    project::write(&root.join(project::META), &json!({"pretty_name": name, "segments": []}))?;
    project::write(&root.join(project::CONFIG), &default_config())?;
    Ok(root)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Still {
    aspect_ratio: Option<String>,
    background: StillBackground,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StillBackground {
    source: Value,
    blur: f64,
    padding: f64,
    rounding: f64,
    rounding_type: String,
    shadow: f64,
    advanced_shadow: Option<project::Shadow>,
    crop: Option<Crop>,
    border: Option<Border>,
}

#[derive(Deserialize, Clone, Copy)]
struct XY {
    x: f64,
    y: f64,
}

#[derive(Deserialize, Clone, Copy)]
struct Crop {
    position: XY,
    size: XY,
}

#[derive(Deserialize)]
#[serde(default)]
struct Border {
    enabled: bool,
    width: f64,
    color: [u8; 3],
    opacity: f64,
}

impl Default for Border {
    fn default() -> Self {
        Self { enabled: false, width: 5.0, color: [0, 0, 0], opacity: 50.0 }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub frame_w: usize,
    pub frame_h: usize,
    /// Where the (cropped) screenshot sits in the frame, in frame pixels.
    pub image: Rect,
    /// The part of original.png that is shown, in its pixels.
    #[serde(skip)]
    crop: Rect,
}

/// Cap's screenshot frame: padding is 0.4 x padding% of the image on each side; with an aspect
/// ratio the padded image is letterboxed into the smallest frame of that ratio. Even sizes.
fn layout(st: &Still, orig_w: usize, orig_h: usize) -> Layout {
    let (ow, oh) = (orig_w as f64, orig_h as f64);
    let crop = match st.background.crop {
        Some(c) if c.size.x >= 1.0 && c.size.y >= 1.0 => {
            let (x, y) = (c.position.x.clamp(0.0, ow - 1.0), c.position.y.clamp(0.0, oh - 1.0));
            Rect { x, y, w: c.size.x.min(ow - x), h: c.size.y.min(oh - y) }
        }
        _ => Rect { x: 0.0, y: 0.0, w: ow, h: oh },
    };
    let (iw, ih) = (crop.w.round().max(1.0), crop.h.round().max(1.0));
    let v = st.background.padding.max(0.0) / 100.0 * 0.4;
    // Round up to even, ignoring float noise (1000 * 1.16 is 1160.0000000000002).
    let even = |x: f64| (((x - 1e-6).ceil() as usize) + 1) & !1;
    let ratio = match st.aspect_ratio.as_deref() {
        Some("wide") => Some(16.0 / 9.0),
        Some("vertical") => Some(9.0 / 16.0),
        Some("square") => Some(1.0),
        Some("classic") => Some(4.0 / 3.0),
        Some("tall") => Some(3.0 / 4.0),
        _ => None,
    };
    let (fw, fh) = match ratio {
        None => (even(iw * (1.0 + 2.0 * v)), even(ih * (1.0 + 2.0 * v))),
        Some(r) => {
            let pad = iw.max(ih) * v;
            let (bw, bh) = (iw + 2.0 * pad, ih + 2.0 * pad);
            if bw / bh > r { (even(bw), even(bw / r)) } else { (even(bh * r), even(bh)) }
        }
    };
    let image = Rect { x: ((fw as f64 - iw) / 2.0).round(), y: ((fh as f64 - ih) / 2.0).round(), w: iw, h: ih };
    Layout { frame_w: fw, frame_h: fh, image, crop }
}

fn parse(config: &Value) -> Result<Still> {
    serde_json::from_value(config.clone()).context("bad screenshot config")
}

/// The background as straight-alpha RGBA at `w`x`h`.
fn background(src: &Value, blur: f64, w: usize, h: usize) -> Result<Image> {
    let mut img = match src.get("type").and_then(Value::as_str) {
        Some("color") => {
            let rgb: [u8; 3] = serde_json::from_value(src["value"].clone()).unwrap_or([255, 255, 255]);
            let a = src.get("alpha").and_then(Value::as_u64).unwrap_or(255).min(255) as u8;
            Image { w, h, px: [rgb[0], rgb[1], rgb[2], a].repeat(w * h) }
        }
        // Gradients and images: reuse the video renderer's background.
        _ => match serde_json::from_value::<BgSource>(src.clone()) {
            Ok(source @ (BgSource::Gradient { .. } | BgSource::Image { path: Some(_) } | BgSource::Wallpaper { path: Some(_) })) => {
                let mut cfg = Config::default();
                cfg.background.source = source;
                render::background(&cfg, w, h)?
            }
            _ => Image { w, h, px: vec![0; w * h * 4] },
        },
    };
    if blur > 0.0 {
        let buf = image::RgbaImage::from_raw(w as u32, h as u32, img.px).context("background size")?;
        img.px = image::imageops::fast_blur(&buf, (blur / 100.0 * w.min(h) as f64 / 20.0).max(0.1) as f32).into_raw();
    }
    Ok(img)
}

/// "Over" for premultiplied colour + alpha.
fn over(c: &mut [f64; 4], rgb: [f64; 3], a: f64) {
    for k in 0..3 {
        c[k] = rgb[k] * a + c[k] * (1.0 - a);
    }
    c[3] = a + c[3] * (1.0 - a);
}

/// The frame without annotations, scaled down to at most `max_w` pixels wide.
pub fn render_frame(config: &Value, original: &Image, max_w: Option<usize>) -> Result<Image> {
    let st = parse(config)?;
    let l = layout(&st, original.w, original.h);
    let k = max_w.map_or(1.0, |m| (m as f64 / l.frame_w as f64).min(1.0));
    let (w, h) = if k < 1.0 { (((l.frame_w as f64 * k).round() as usize).max(1), ((l.frame_h as f64 * k).round() as usize).max(1)) } else { (l.frame_w, l.frame_h) };
    let b = &st.background;
    let bg = background(&b.source, b.blur, w, h)?;
    let card = Rect { x: l.image.x * k, y: l.image.y * k, w: l.image.w * k, h: l.image.h * k };
    let (cx, cy, hw, hh) = (card.x + card.w / 2.0, card.y + card.h / 2.0, card.w / 2.0, card.h / 2.0);
    let half_min = hw.min(hh);
    let radius = b.rounding.max(0.0) / 100.0 * half_min;
    let squircle = b.rounding_type != "rounded";
    // Same shadow law as the video renderer (and Cap's shader).
    let s = b.shadow / 100.0;
    let adv = b.advanced_shadow.clone().unwrap_or_default();
    let (sh_size, sh_opacity, sh_blur) = (s * adv.size / 100.0 * half_min, s * adv.opacity / 100.0, s * adv.blur / 100.0 * half_min);
    let border = b.border.as_ref().filter(|b| b.enabled && b.width > 0.0).map(|b| (b.width * k, b.color.map(|c| c as f64 / 255.0), b.opacity / 100.0));
    let crop = l.crop;
    let (du, dv) = (crop.w / original.w as f64 / card.w, crop.h / original.h as f64 / card.h);

    let mut px = vec![0u8; w * h * 4];
    px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = bg.px[i + 3] as f64 / 255.0;
            let mut c = [bg.px[i] as f64 / 255.0 * a, bg.px[i + 1] as f64 / 255.0 * a, bg.px[i + 2] as f64 / 255.0 * a, a];
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let d = render::sdf(fx - cx, fy - cy, hw, hh, radius, squircle);
            if sh_opacity > 0.0 && d > 0.0 {
                over(&mut c, [0.0; 3], render::smoothstep(sh_size + sh_blur, -sh_blur, d) * sh_opacity);
            }
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover > 0.0 {
                let u = (crop.x + (fx - card.x) / card.w * crop.w) / original.w as f64;
                let v = (crop.y + (fy - card.y) / card.h * crop.h) / original.h as f64;
                let p = render::sample(original, u, v, du, dv);
                over(&mut c, [p[0], p[1], p[2]], p[3] * cover);
            }
            if let Some((bw, rgb, op)) = border {
                // A ring just outside the image edge.
                let ring = ((0.5 + d).clamp(0.0, 1.0) * (bw + 0.5 - d).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                if ring > 0.0 {
                    over(&mut c, rgb, ring * op);
                }
            }
            for k in 0..3 {
                row[x * 4 + k] = if c[3] > 0.0 { (c[k] / c[3] * 255.0 + 0.5).min(255.0) as u8 } else { 0 };
            }
            row[x * 4 + 3] = (c[3] * 255.0 + 0.5) as u8;
        }
    });
    Ok(Image { w, h, px })
}

fn png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Adaptive)
        .write_image(&img.px, img.w as u32, img.h as u32, image::ExtendedColorType::Rgba8)?;
    Ok(out)
}

pub fn load_image(path: &Path) -> Result<Image> {
    let img = image::open(path).with_context(|| format!("opening {}", path.display()))?.to_rgba8();
    Ok(Image { w: img.width() as usize, h: img.height() as usize, px: img.into_raw() })
}

/// A JPEG preview of a screenshot's original image, at most `max_w` wide.
pub fn thumbnail(root: &Path, max_w: u32) -> Result<Vec<u8>> {
    let img = image::open(root.join(ORIGINAL))?;
    let img = if img.width() > max_w { img.thumbnail(max_w, max_w * 4) } else { img };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80).encode_image(&img.to_rgb8())?;
    Ok(out)
}

// ---------- editor window ----------

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

struct Shot {
    root: PathBuf,
    name: String,
    original: Image,
}

#[derive(Serialize)]
struct ShotInfo {
    path: String,
    name: String,
    config: Value,
    width: usize,
    height: usize,
}

fn config_on_disk(root: &Path) -> Value {
    project::read(&root.join(project::CONFIG)).unwrap_or_else(|_| default_config())
}

#[tauri::command]
fn ss_project(st: State<Shot>) -> ShotInfo {
    let mut config = config_on_disk(&st.root);
    // Fill in what an older or foreign config lacks, keeping everything else.
    let mut full = default_config();
    crate::editor::merge(&mut full, config.take());
    ShotInfo { path: st.root.display().to_string(), name: st.name.clone(), config: full, width: st.original.w, height: st.original.h }
}

#[tauri::command]
fn ss_layout(st: State<Shot>, config: Value) -> CmdResult<Layout> {
    Ok(layout(&parse(&config).map_err(err)?, st.original.w, st.original.h))
}

#[tauri::command]
async fn ss_frame(st: State<'_, Shot>, config: Value, max_width: usize) -> CmdResult<Response> {
    let img = render_frame(&config, &st.original, Some(max_width.max(16))).map_err(err)?;
    png(&img).map(Response::new).map_err(err)
}

#[tauri::command]
async fn ss_render(st: State<'_, Shot>, config: Value) -> CmdResult<Response> {
    let img = render_frame(&config, &st.original, None).map_err(err)?;
    png(&img).map(Response::new).map_err(err)
}

#[tauri::command]
fn ss_save(st: State<Shot>, config: Value) -> CmdResult<()> {
    let mut disk = config_on_disk(&st.root);
    crate::editor::merge(&mut disk, config);
    project::write(&st.root.join(project::CONFIG), &disk).map_err(err)
}

fn raw(request: &Request) -> CmdResult<Vec<u8>> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        _ => Err("expected raw PNG bytes".into()),
    }
}

#[tauri::command]
fn ss_copy_png(request: Request) -> CmdResult<()> {
    let tmp = crate::os::temp_file("png").map_err(err)?;
    std::fs::write(&tmp, raw(&request)?).map_err(err)?;
    let res = crate::os::copy_png(&tmp).map_err(err);
    // macOS reads the file synchronously; the other tools have read it by the time they exit or fork.
    let _ = std::fs::remove_file(&tmp);
    res
}

#[tauri::command]
fn ss_save_png(st: State<Shot>, request: Request) -> CmdResult<String> {
    let path = crate::os::unique(&crate::os::media_dir(true), &st.name, "png");
    std::fs::write(&path, raw(&request)?).map_err(err)?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn ss_reveal(st: State<Shot>) -> CmdResult<()> {
    crate::os::reveal(&st.root).map_err(err)
}

#[tauri::command]
fn ss_delete(app: tauri::AppHandle, st: State<Shot>) -> CmdResult<()> {
    std::fs::remove_dir_all(&st.root).map_err(err)?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn save_asset(st: State<Shot>, request: Request) -> CmdResult<String> {
    let name = request.headers().get("name").and_then(|v| v.to_str().ok()).unwrap_or("background.png");
    let name = Path::new(name).file_name().ok_or("bad file name")?.to_owned();
    let dir = st.root.join("assets");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join(name);
    std::fs::write(&path, raw(&request)?).map_err(err)?;
    Ok(path.display().to_string())
}

/// The screenshot's text lines, for selecting and copying text over the image.
#[tauri::command]
async fn ss_ocr(st: State<'_, Shot>) -> CmdResult<Value> {
    let lines = crate::ocr::recognize(&st.root.join(ORIGINAL)).map_err(err)?;
    Ok(json!({ "lines": lines }))
}

#[tauri::command]
fn log(msg: String) {
    eprintln!("[ui] {msg}");
}

pub fn run(root: &Path) -> Result<()> {
    let original = load_image(&root.join(ORIGINAL))?;
    let meta: Value = project::read(&root.join(project::META)).unwrap_or_default();
    let name = meta["pretty_name"].as_str().filter(|n| !n.trim().is_empty()).map(String::from).unwrap_or_else(|| root.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or("Screenshot".into()));
    let (w, h) = (original.w as f64, original.h as f64);
    tauri::Builder::default()
        .manage(Shot { root: root.to_path_buf(), name, original })
        .invoke_handler(tauri::generate_handler![ss_project, ss_layout, ss_frame, ss_render, ss_save, ss_copy_png, ss_save_png, ss_reveal, ss_delete, save_asset, ss_ocr, log])
        .setup(move |app| {
            // Fit the screenshot comfortably on the primary screen.
            let (sw, sh) = app.primary_monitor()?.map_or((1440.0, 900.0), |m| (m.size().width as f64 / m.scale_factor(), m.size().height as f64 / m.scale_factor()));
            let k = (sw * 0.8 / w).min((sh * 0.8 - 56.0) / h).min(1.0);
            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("screenshot.html".into()))
                .title("Precap Screenshot Editor")
                .inner_size((w * k).max(900.0), (h * k + 56.0).max(560.0))
                .min_inner_size(760.0, 480.0)
                .theme(Some(tauri::Theme::Dark));
            #[cfg(target_os = "macos")]
            let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true).traffic_light_position(tauri::LogicalPosition::new(20.0, 22.0));
            builder.build()?.set_focus()?;
            Ok(())
        })
        .run(crate::editor::context())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still(v: Value) -> Still {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn frame_math_matches_cap() {
        // padding 20 -> 8% of the image on each side.
        let l = layout(&still(json!({"background": {"padding": 20}})), 1000, 500);
        assert_eq!((l.frame_w, l.frame_h), (1160, 580));
        assert_eq!(l.image, Rect { x: 80.0, y: 40.0, w: 1000.0, h: 500.0 });
        // 16:9 letterboxes the padded image (1160 x 660) into 1174 x 660.
        let l = layout(&still(json!({"aspectRatio": "wide", "background": {"padding": 20}})), 1000, 500);
        assert_eq!((l.frame_w, l.frame_h), (1174, 660));
        assert_eq!((l.image.w, l.image.h), (1000.0, 500.0));
        // Crop changes what's shown and the frame with it.
        let l = layout(&still(json!({"background": {"padding": 0, "crop": {"position": {"x": 10, "y": 20}, "size": {"x": 300, "y": 200}}}})), 1000, 500);
        assert_eq!((l.frame_w, l.frame_h, l.crop), (300, 200, Rect { x: 10.0, y: 20.0, w: 300.0, h: 200.0 }));
    }

    #[test]
    fn renders_transparent_padding_and_opaque_image() {
        let original = Image { w: 100, h: 50, px: [200, 10, 10, 255].repeat(5000) };
        let cfg = json!({"background": {"source": {"type": "color", "value": [255, 255, 255], "alpha": 0}, "padding": 20, "rounding": 0}});
        let img = render_frame(&cfg, &original, None).unwrap();
        assert_eq!((img.w, img.h), (116, 58));
        let at = |x: usize, y: usize| &img.px[(y * img.w + x) * 4..(y * img.w + x) * 4 + 4];
        assert_eq!(at(2, 2)[3], 0, "padding stays transparent");
        assert_eq!(at(58, 29), &[200, 10, 10, 255], "image pixels come through");
        let small = render_frame(&cfg, &original, Some(58)).unwrap();
        assert_eq!((small.w, small.h), (58, 29));
    }
}
