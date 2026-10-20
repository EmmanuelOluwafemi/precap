//! Per-pixel compositor: background → shadow → rounded screen card (zoomed via
//! crop) → cursor → camera bubble. A CPU port of the ideas in Cap's
//! `composite-video-frame.wgsl`: same squircle SDF and the same shadow formula.

use crate::project::{BgSource, Config};
use anyhow::Result;
use rayon::prelude::*;

/// Straight-alpha RGBA8 image.
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub struct Sprite {
    pub img: Image,
    /// Hotspot as a fraction of the image size.
    pub hot: (f64, f64),
}

/// Everything that moves from frame to frame.
pub struct Frame<'a> {
    pub screen: &'a Image,
    /// Already cropped square by the decoder.
    pub camera: Option<&'a Image>,
    /// Visible part of the screen, normalized (zoom = smaller crop).
    pub crop: Rect,
    /// Cursor position normalized to the screen.
    pub cursor: Option<((f64, f64), &'a Sprite)>,
}

/// Everything fixed for the whole export.
pub struct Layout {
    pub w: usize,
    pub card: Rect,
    pub card_radius: f64,
    pub card_squircle: bool,
    pub shadow: (f64, f64, f64), // size, opacity, blur (px, 0..1, px)
    pub cam: Option<Rect>,
    pub cam_radius: f64,
    pub cam_squircle: bool,
    pub cam_mirror: bool,
    pub cursor_height: f64, // px at zoom 1
}

pub fn layout(cfg: &Config, w: usize, h: usize, src_aspect: f64, has_camera: bool) -> Layout {
    let short = w.min(h) as f64;
    let pad = cfg.background.padding / 100.0 * short;
    let (aw, ah) = (w as f64 - 2.0 * pad, h as f64 - 2.0 * pad);
    let (cw, ch) = if aw / ah > src_aspect { (ah * src_aspect, ah) } else { (aw, aw / src_aspect) };
    let card = Rect { x: (w as f64 - cw) / 2.0, y: (h as f64 - ch) / 2.0, w: cw, h: ch };

    // Cap's shadow math: everything scales with the master `shadow` strength and the card's half-size.
    let half_min = cw.min(ch) / 2.0;
    let s = cfg.background.shadow / 100.0;
    let adv = &cfg.background.advanced_shadow;
    let shadow = (s * adv.size / 100.0 * half_min, s * adv.opacity / 100.0, s * adv.blur / 100.0 * half_min);

    let c = &cfg.camera;
    let cam = (has_camera && !c.hide).then(|| {
        let side = c.size / 100.0 * short;
        let margin = 0.03 * short;
        let x = match c.position.x.as_str() {
            "left" => margin,
            "center" => (w as f64 - side) / 2.0,
            _ => w as f64 - side - margin,
        };
        let y = if c.position.y == "top" { margin } else { h as f64 - side - margin };
        match c.manual_position {
            Some(m) => Rect { x: m.x * w as f64 - side / 2.0, y: m.y * h as f64 - side / 2.0, w: side, h: side },
            None => Rect { x, y, w: side, h: side },
        }
    });

    Layout {
        w,
        card,
        card_radius: cfg.background.rounding / 100.0 * half_min,
        card_squircle: cfg.background.rounding_type != "rounded",
        shadow,
        cam,
        cam_radius: c.rounding / 100.0 * c.size / 200.0 * short,
        cam_squircle: c.rounding_type != "rounded",
        cam_mirror: c.mirror,
        cursor_height: 0.042 * card.h * cfg.cursor.size / 100.0,
    }
}

/// Renders the background once; it never changes during an export.
pub fn background(cfg: &Config, w: usize, h: usize) -> Result<Image> {
    let mut px = vec![255u8; w * h * 4];
    match &cfg.background.source {
        BgSource::Wallpaper { path: Some(p) } | BgSource::Image { path: Some(p) } => match load_background(p) {
            // Cover the frame (crop, don't stretch), like a desktop wallpaper.
            Ok(img) => px.copy_from_slice(img.resize_to_fill(w as u32, h as u32, image::imageops::FilterType::Triangle).to_rgba8().as_raw()),
            Err(e) => {
                // A missing file (e.g. one of Cap's own wallpapers) shouldn't stop previews or exports.
                eprintln!("background image {p}: {e:#}");
                px.chunks_mut(4).for_each(|p| p[..3].copy_from_slice(&[24, 24, 27]));
            }
        },
        BgSource::Color { value } => px.chunks_mut(4).for_each(|p| p[..3].copy_from_slice(value)),
        BgSource::Gradient { from, to, angle } => {
            let (dx, dy) = (angle.to_radians().sin(), -angle.to_radians().cos());
            let span = (w as f64 * dx.abs() + h as f64 * dy.abs()).max(1.0);
            for (i, p) in px.chunks_mut(4).enumerate() {
                let (x, y) = ((i % w) as f64 - w as f64 / 2.0, (i / w) as f64 - h as f64 / 2.0);
                let t = ((x * dx + y * dy) / span + 0.5).clamp(0.0, 1.0);
                for k in 0..3 {
                    p[k] = (from[k] as f64 + (to[k] as f64 - from[k] as f64) * t) as u8;
                }
            }
        }
        _ => px.chunks_mut(4).for_each(|p| p[..3].copy_from_slice(&[24, 24, 27])),
    }
    Ok(Image { w, h, px })
}

/// A file on disk, or one of our built-in wallpapers ("wallpapers/<category>/<n>.jpg", embedded in the app).
fn load_background(path: &str) -> Result<image::DynamicImage> {
    if !std::path::Path::new(path).is_absolute() && path.starts_with("wallpapers/") {
        let ctx = crate::editor::context();
        let bytes = ctx.assets().get(&path.into()).ok_or_else(|| anyhow::anyhow!("no built-in wallpaper {path}"))?;
        return Ok(image::load_from_memory(&bytes)?);
    }
    Ok(image::open(path)?)
}

/// Signed distance to a rounded rect centred at the origin (negative inside).
/// Squircle corners use a superellipse norm with n = 4, like Cap.
pub fn sdf(px: f64, py: f64, half_w: f64, half_h: f64, r: f64, squircle: bool) -> f64 {
    let r = r.min(half_w).min(half_h);
    let (qx, qy) = (px.abs() - half_w + r, py.abs() - half_h + r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    let norm = if squircle { (ox.powi(4) + oy.powi(4)).powf(0.25) } else { (ox * ox + oy * oy).sqrt() };
    norm + qx.max(qy).min(0.0) - r
}

/// Cap passes edge0 > edge1 here; this works either way round.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    // A shadow with no size and no blur covers nothing.
    if e0 == e1 {
        return 0.0;
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bilinear(img: &Image, u: f64, v: f64) -> [f64; 4] {
    let x = (u * img.w as f64 - 0.5).clamp(0.0, (img.w - 1) as f64);
    let y = (v * img.h as f64 - 0.5).clamp(0.0, (img.h - 1) as f64);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(img.w - 1), (y0 + 1).min(img.h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let at = |x: usize, y: usize, k: usize| img.px[(y * img.w + x) * 4 + k] as f64;
    let mut out = [0.0; 4];
    for (k, o) in out.iter_mut().enumerate() {
        let top = at(x0, y0, k) * (1.0 - fx) + at(x1, y0, k) * fx;
        let bot = at(x0, y1, k) * (1.0 - fx) + at(x1, y1, k) * fx;
        *o = (top * (1.0 - fy) + bot * fy) / 255.0;
    }
    out
}

/// Bilinear, plus a 4-tap average over the pixel footprint when shrinking,
/// so small text doesn't shimmer (Cap's shader does the same trick).
pub fn sample(img: &Image, u: f64, v: f64, du: f64, dv: f64) -> [f64; 4] {
    if du * img.w as f64 <= 1.1 {
        return bilinear(img, u, v);
    }
    let (a, b) = (du / 4.0, dv / 4.0);
    let taps = [bilinear(img, u - a, v - b), bilinear(img, u + a, v - b), bilinear(img, u - a, v + b), bilinear(img, u + a, v + b)];
    std::array::from_fn(|k| taps.iter().map(|t| t[k]).sum::<f64>() / 4.0)
}

fn over(dst: &mut [f64; 3], src: [f64; 3], alpha: f64) {
    for k in 0..3 {
        dst[k] += (src[k] - dst[k]) * alpha;
    }
}

pub fn compose(out: &mut [u8], bg: &Image, l: &Layout, f: &Frame) {
    let card = l.card;
    let (cx, cy, hw, hh) = (card.x + card.w / 2.0, card.y + card.h / 2.0, card.w / 2.0, card.h / 2.0);
    let (shadow_size, shadow_opacity, shadow_blur) = l.shadow;
    // Footprint of one output pixel in screen-texture UV space.
    let (du, dv) = (f.crop.w / card.w, f.crop.h / card.h);

    // Cursor sprite placement in output pixels; it grows with zoom like the screen does.
    let cursor = f.cursor.map(|((nx, ny), sprite)| {
        let h = l.cursor_height / f.crop.h;
        let w = h * sprite.img.w as f64 / sprite.img.h as f64;
        let x = card.x + (nx - f.crop.x) / f.crop.w * card.w - sprite.hot.0 * w;
        let y = card.y + (ny - f.crop.y) / f.crop.h * card.h - sprite.hot.1 * h;
        (Rect { x, y, w, h }, sprite)
    });

    out.par_chunks_mut(l.w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..l.w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let i = (y * l.w + x) * 4;
            let mut c = [bg.px[i] as f64 / 255.0, bg.px[i + 1] as f64 / 255.0, bg.px[i + 2] as f64 / 255.0];

            let d = sdf(px - cx, py - cy, hw, hh, l.card_radius, l.card_squircle);
            if shadow_opacity > 0.0 {
                over(&mut c, [0.0; 3], smoothstep(shadow_size + shadow_blur, -shadow_blur, d.abs()) * shadow_opacity);
            }
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover > 0.0 {
                let u = f.crop.x + (px - card.x) / card.w * f.crop.w;
                let v = f.crop.y + (py - card.y) / card.h * f.crop.h;
                let s = sample(f.screen, u, v, du, dv);
                over(&mut c, [s[0], s[1], s[2]], cover);
            }

            if let Some((r, sprite)) = &cursor {
                if px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h {
                    let s = sample(&sprite.img, (px - r.x) / r.w, (py - r.y) / r.h, 1.0 / r.w, 1.0 / r.h);
                    over(&mut c, [s[0], s[1], s[2]], s[3]);
                }
            }

            if let (Some(cam), Some(img)) = (l.cam, f.camera) {
                let half = cam.w / 2.0;
                let d = sdf(px - cam.x - half, py - cam.y - half, half, half, l.cam_radius, l.cam_squircle);
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let u = (px - cam.x) / cam.w;
                    let u = if l.cam_mirror { 1.0 - u } else { u };
                    let s = sample(img, u, (py - cam.y) / cam.h, 1.0 / cam.w, 1.0 / cam.h);
                    over(&mut c, [s[0], s[1], s[2]], cover);
                }
            }

            for k in 0..3 {
                row[x * 4 + k] = (c[k] * 255.0 + 0.5) as u8;
            }
            row[x * 4 + 3] = 255;
        }
    });
}

/// Crops transparent margins so every cursor image is sized by its visible shape
/// (Cap's saved cursor PNGs range from 32x32 to 280x400 with lots of padding).
pub fn trim(s: Sprite) -> Sprite {
    let Image { w, h, px } = &s.img;
    let (mut x0, mut y0, mut x1, mut y1) = (*w, *h, 0, 0);
    for y in 0..*h {
        for x in 0..*w {
            if px[(y * w + x) * 4 + 3] > 25 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x1 <= x0 {
        return s;
    }
    let (nw, nh) = (x1 - x0, y1 - y0);
    let rows = (y0..y1).flat_map(|y| px[(y * w + x0) * 4..(y * w + x1) * 4].iter().copied()).collect();
    let hot = ((s.hot.0 * *w as f64 - x0 as f64) / nw as f64, (s.hot.1 * *h as f64 - y0 as f64) / nh as f64);
    Sprite { img: Image { w: nw, h: nh, px: rows }, hot }
}

/// Classic arrow, used when a project has no captured cursor images (ours never do).
pub fn arrow_sprite() -> Sprite {
    const PTS: [(f64, f64); 7] = [(0.08, 0.04), (0.08, 0.80), (0.27, 0.63), (0.40, 0.94), (0.53, 0.88), (0.40, 0.58), (0.64, 0.58)];
    let (w, h) = (48usize, 64usize);
    let inside = |x: f64, y: f64| {
        let mut odd = false;
        for i in 0..PTS.len() {
            let ((x1, y1), (x2, y2)) = (PTS[i], PTS[(i + 1) % PTS.len()]);
            if (y1 > y) != (y2 > y) && x < x1 + (y - y1) / (y2 - y1) * (x2 - x1) {
                odd = !odd;
            }
        }
        odd
    };
    let edge = |x: f64, y: f64| {
        (0..PTS.len())
            .map(|i| {
                let ((x1, y1), (x2, y2)) = (PTS[i], PTS[(i + 1) % PTS.len()]);
                let (dx, dy) = (x2 - x1, y2 - y1);
                let t = (((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                ((x - x1 - t * dx).powi(2) + (y - y1 - t * dy).powi(2)).sqrt()
            })
            .fold(f64::MAX, f64::min)
    };
    let mut px = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            // 4x4 supersampling for smooth edges; a thin black outline around a white fill.
            let (mut a, mut white) = (0.0, 0.0);
            for s in 0..16 {
                let (sx, sy) = ((x as f64 + (s % 4) as f64 / 4.0 + 0.125) / h as f64, (y as f64 + (s / 4) as f64 / 4.0 + 0.125) / h as f64);
                if inside(sx, sy) {
                    a += 1.0 / 16.0;
                    if edge(sx, sy) > 0.035 {
                        white += 1.0 / 16.0;
                    }
                }
            }
            let v = if a > 0.0 { (white / a * 255.0) as u8 } else { 0 };
            px[(y * w + x) * 4..][..4].copy_from_slice(&[v, v, v, (a * 255.0) as u8]);
        }
    }
    Sprite { img: Image { w, h, px }, hot: (0.08 * h as f64 / w as f64, 0.04) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdf_signs() {
        assert!(sdf(0.0, 0.0, 10.0, 5.0, 2.0, true) < 0.0);
        assert!((sdf(10.0, 0.0, 10.0, 5.0, 2.0, true)).abs() < 1e-9);
        assert!(sdf(11.0, 0.0, 10.0, 5.0, 2.0, false) > 0.0);
        // A squircle corner bulges further out than a circular one.
        assert!(sdf(9.5, 4.5, 10.0, 5.0, 2.0, true) < sdf(9.5, 4.5, 10.0, 5.0, 2.0, false));
    }

    #[test]
    fn trim_keeps_hotspot_on_same_pixel() {
        let mut px = vec![0u8; 10 * 10 * 4];
        px[(5 * 10 + 4) * 4 + 3] = 255; // one visible pixel at (4, 5)
        let s = trim(Sprite { img: Image { w: 10, h: 10, px }, hot: (0.45, 0.55) });
        assert_eq!((s.img.w, s.img.h), (1, 1));
        assert!((s.hot.0 - 0.5).abs() < 1e-9 && (s.hot.1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn layout_fits_card_inside_padding() {
        let cfg = Config::default();
        let l = layout(&cfg, 1920, 1080, 16.0 / 10.0, true);
        assert!(l.card.x >= 0.0 && l.card.y > 0.0 && l.card.x + l.card.w <= 1920.0);
        assert!((l.card.w / l.card.h - 1.6).abs() < 1e-9);
        assert!(l.cam.unwrap().x + l.cam.unwrap().w < 1920.0);
    }
}
