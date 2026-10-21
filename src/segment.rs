//! Camera background blur / removal: a person matte (Apple Vision on macOS; other
//! systems have none and show the camera untouched) plus a cheap CPU composite.

use crate::project::BlurMode;
use crate::render::{Image, sample, smoothstep};
use rayon::prelude::*;

/// Long side Vision sees; the matte is soft anyway and gets upsampled bilinearly.
#[cfg(target_os = "macos")]
const SEG_SIDE: usize = 256;

/// Person matte, 0..255 in the red channel (an `Image` so `sample` can upsample it).
#[cfg(target_os = "macos")]
pub fn person_mask(img: &Image) -> anyhow::Result<Image> {
    use anyhow::{Context, bail};
    use objc2::AnyThread;
    use objc2::rc::Retained;
    use objc2_core_video::*;
    use objc2_foundation::{NSArray, NSDictionary};
    use objc2_vision::{VNGeneratePersonSegmentationRequest, VNGeneratePersonSegmentationRequestQualityLevel, VNImageRequestHandler, VNRequest};
    use std::ptr::{NonNull, null_mut};

    let s = (SEG_SIDE as f64 / img.w.max(img.h) as f64).min(1.0);
    let (w, h) = (((img.w as f64 * s) as usize).max(1), ((img.h as f64 * s) as usize).max(1));
    let mut buf: *mut CVPixelBuffer = null_mut();
    // 0x42475241 = 'BGRA'
    if unsafe { CVPixelBufferCreate(None, w, h, 0x4247_5241, None, NonNull::from(&mut buf)) } != 0 {
        bail!("no pixel buffer");
    }
    // Create rule: we own the +1 reference.
    let input = unsafe { Retained::from_raw(buf) }.context("no pixel buffer")?;
    unsafe { CVPixelBufferLockBaseAddress(&input, CVPixelBufferLockFlags(0)) };
    let (base, stride) = (CVPixelBufferGetBaseAddress(&input) as *mut u8, CVPixelBufferGetBytesPerRow(&input));
    for y in 0..h {
        let row = unsafe { std::slice::from_raw_parts_mut(base.add(y * stride), w * 4) };
        for (x, p) in row.chunks_exact_mut(4).enumerate() {
            let c = sample(img, (x as f64 + 0.5) / w as f64, (y as f64 + 0.5) / h as f64, 1.0 / w as f64, 1.0 / h as f64);
            p.copy_from_slice(&[(c[2] * 255.0) as u8, (c[1] * 255.0) as u8, (c[0] * 255.0) as u8, 255]);
        }
    }
    unsafe { CVPixelBufferUnlockBaseAddress(&input, CVPixelBufferLockFlags(0)) };

    let handler = unsafe { VNImageRequestHandler::initWithCVPixelBuffer_options(VNImageRequestHandler::alloc(), &input, &NSDictionary::new()) };
    let request = unsafe { VNGeneratePersonSegmentationRequest::new() };
    unsafe {
        request.setQualityLevel(VNGeneratePersonSegmentationRequestQualityLevel::Balanced);
        request.setOutputPixelFormat(kCVPixelFormatType_OneComponent8);
    }
    let as_request: Retained<VNRequest> = Retained::into_super(Retained::into_super(Retained::into_super(request.clone())));
    handler.performRequests_error(&NSArray::from_retained_slice(&[as_request])).map_err(|e| anyhow::anyhow!("person segmentation failed: {e}"))?;
    let results = unsafe { request.results() }.context("no segmentation results")?;
    let out = results.firstObject().context("no segmentation mask")?;
    let mask = unsafe { out.pixelBuffer() };
    unsafe { CVPixelBufferLockBaseAddress(&mask, CVPixelBufferLockFlags::ReadOnly) };
    let (mw, mh, stride) = (CVPixelBufferGetWidth(&mask), CVPixelBufferGetHeight(&mask), CVPixelBufferGetBytesPerRow(&mask));
    let base = CVPixelBufferGetBaseAddress(&mask) as *const u8;
    let mut px = vec![255u8; mw * mh * 4];
    for y in 0..mh {
        let row = unsafe { std::slice::from_raw_parts(base.add(y * stride), mw) };
        for (x, &v) in row.iter().enumerate() {
            px[(y * mw + x) * 4] = v;
        }
    }
    unsafe { CVPixelBufferUnlockBaseAddress(&mask, CVPixelBufferLockFlags::ReadOnly) };
    Ok(Image { w: mw, h: mh, px })
}

#[cfg(not(target_os = "macos"))]
pub fn person_mask(_: &Image) -> anyhow::Result<Image> {
    anyhow::bail!("person segmentation is only available on macOS")
}

/// In-place separable box blur of the colour channels, `passes` times (three approximate a gaussian). Edges clamp.
fn box_blur(img: &mut Image, r: usize, passes: usize) {
    let (w, h) = (img.w, img.h);
    let n = (2 * r + 1) as u32;
    let line = |idx: &dyn Fn(usize) -> usize, len: usize, px: &mut [u8], k: usize| {
        let src: Vec<u8> = (0..len).map(|i| px[idx(i) * 4 + k]).collect();
        let at = |i: isize| src[i.clamp(0, len as isize - 1) as usize] as u32;
        let mut sum: u32 = (-(r as isize)..=r as isize).map(at).sum();
        for i in 0..len as isize {
            px[idx(i as usize) * 4 + k] = ((sum + n / 2) / n) as u8;
            sum = sum + at(i + r as isize + 1) - at(i - r as isize);
        }
    };
    for _ in 0..passes {
        for k in 0..3 {
            for y in 0..h {
                line(&|x| y * w + x, w, &mut img.px, k);
            }
            for x in 0..w {
                line(&|y| y * w + x, h, &mut img.px, k);
            }
        }
    }
}

/// The camera frame with its background blurred (alpha stays opaque) or cut out (alpha = matte).
pub fn apply(img: &Image, mask: &Image, mode: BlurMode) -> Image {
    let (du, dv) = (1.0 / img.w as f64, 1.0 / img.h as f64);
    // Blur a quarter-size copy: it is cheap and the upsampling smooths it further.
    let blurred = matches!(mode, BlurMode::Light | BlurMode::Heavy).then(|| {
        let (sw, sh) = ((img.w / 4).max(1), (img.h / 4).max(1));
        let mut px = Vec::with_capacity(sw * sh * 4);
        for y in 0..sh {
            for x in 0..sw {
                let c = sample(img, (x as f64 + 0.5) / sw as f64, (y as f64 + 0.5) / sh as f64, 1.0 / sw as f64, 1.0 / sh as f64);
                px.extend(c.map(|v| (v * 255.0 + 0.5) as u8));
            }
        }
        let mut small = Image { w: sw, h: sh, px };
        box_blur(&mut small, if mode == BlurMode::Light { 2 } else { 6 }, 3);
        small
    });
    let mut out = Image { w: img.w, h: img.h, px: img.px.clone() };
    out.px.par_chunks_mut(img.w * 4).enumerate().for_each(|(y, row)| {
        let v = (y as f64 + 0.5) * dv;
        for (x, p) in row.chunks_exact_mut(4).enumerate() {
            let u = (x as f64 + 0.5) * du;
            // Feather: steepen the soft matte into a clean but unjagged edge.
            let m = smoothstep(0.35, 0.65, sample(mask, u, v, du, dv)[0]);
            match &blurred {
                Some(b) => {
                    let bg = sample(b, u, v, du, dv);
                    for k in 0..3 {
                        p[k] = (bg[k] * 255.0 * (1.0 - m) + p[k] as f64 * m + 0.5) as u8;
                    }
                }
                None => p[3] = (m * 255.0 + 0.5) as u8,
            }
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Red left half is the person (matte 255), blue right half the background (matte 0).
    fn fixture(side: usize) -> (Image, Image) {
        let mut img = Image { w: side, h: side, px: vec![255; side * side * 4] };
        // Like Vision's output, the matte is lower resolution than the frame.
        let m = side / 8;
        let mut mask = Image { w: m, h: m, px: vec![255; m * m * 4] };
        for y in 0..side {
            for x in 0..side {
                img.px[(y * side + x) * 4..][..3].copy_from_slice(if x < side / 2 { &[255, 0, 0] } else { &[0, 0, 255] });
            }
        }
        for y in 0..m {
            for x in 0..m {
                mask.px[(y * m + x) * 4] = if x < m / 2 { 255 } else { 0 };
            }
        }
        (img, mask)
    }

    fn at(i: &Image, x: usize, y: usize) -> [u8; 4] {
        i.px[(y * i.w + x) * 4..][..4].try_into().unwrap()
    }

    #[test]
    fn remove_cuts_out_background() {
        let (img, mask) = fixture(64);
        let out = apply(&img, &mask, BlurMode::Remove);
        assert_eq!(at(&out, 4, 32), [255, 0, 0, 255]);
        assert_eq!(at(&out, 60, 32)[3], 0);
        let edge = at(&out, 32, 32)[3];
        assert!(edge > 0 && edge < 255, "edge is feathered: {edge}");
    }

    #[test]
    fn blur_keeps_person_and_softens_background() {
        let (img, mask) = fixture(64);
        let (light, heavy) = (apply(&img, &mask, BlurMode::Light), apply(&img, &mask, BlurMode::Heavy));
        assert_eq!(at(&light, 4, 32), [255, 0, 0, 255]);
        // The person's colour bleeds into the background near the edge, more so when heavy.
        let red = |i: &Image| at(i, 36, 32)[0];
        assert!(red(&heavy) > red(&light) && red(&light) > 0, "heavy {} vs light {}", red(&heavy), red(&light));
    }

    #[test]
    fn unknown_blur_mode_reads_as_off() {
        let read = |s: &str| serde_json::from_str::<BlurMode>(s).unwrap();
        assert_eq!((read("\"sparkles\""), read("\"heavy\"")), (BlurMode::Off, BlurMode::Heavy));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "runs Apple Vision"]
    fn vision_matte_dimensions() {
        let png = image::open(concat!(env!("CARGO_MANIFEST_DIR"), "/src/testdata/ocr.png")).unwrap().to_rgba8();
        let img = Image { w: png.width() as usize, h: png.height() as usize, px: png.into_raw() };
        let t = std::time::Instant::now();
        let mask = person_mask(&img).unwrap();
        eprintln!("vision: {}x{} in {:?}", mask.w, mask.h, t.elapsed());
        let t = std::time::Instant::now();
        person_mask(&img).unwrap();
        eprintln!("vision (warm): {:?}", t.elapsed());
        // Balanced quality gives a fixed 512x384 matte whatever the input shape; it spans the whole frame.
        assert_eq!((mask.w, mask.h), (512, 384));
        assert_eq!(mask.px.len(), mask.w * mask.h * 4);
    }
}
