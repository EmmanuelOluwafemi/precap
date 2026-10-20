//! Local Instant mode: right after stopping, turn the raw tracks into one ready-to-share
//! `content/output.mp4` with the camera bubble baked in where it sat on screen (Cap bakes
//! the camera "with its preview appearance" too). One fast ffmpeg pass per segment, no
//! per-pixel rendering, so it's ready in seconds; the OS cursor was captured live.

use crate::project::{self, Meta, Track};
use crate::record::probe;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the camera bubble goes, as fractions of the video frame.
#[derive(Clone, Copy, Debug)]
pub struct Bubble {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub round: bool,
    pub mirror: bool,
}

pub const OUTPUT: &str = "content/output.mp4";

/// Starts a stream `d` seconds after the display (negative = it started first, so cut its head).
fn shift(d: f64, audio: bool) -> String {
    match (d >= 0.0, audio) {
        (true, false) => format!("setpts=PTS-STARTPTS+{d:.6}/TB"),
        (true, true) => format!("asetpts=PTS-STARTPTS,adelay={:.0}:all=1", d * 1000.0),
        (false, false) => format!("trim=start={:.6},setpts=PTS-STARTPTS", -d),
        (false, true) => format!("atrim=start={:.6},asetpts=PTS-STARTPTS", -d),
    }
}

/// Alpha for the bubble: a circle, or a rounded rectangle like the camera window's square/wide shapes.
fn bubble_alpha(round: bool) -> &'static str {
    if round {
        "if(lte(hypot(X-W/2,Y-H/2),min(W,H)/2),255,0)"
    } else {
        "if(gt(abs(X-W/2),W/2-min(W,H)*0.16)*gt(abs(Y-H/2),H/2-min(W,H)*0.16)*gt(hypot(abs(X-W/2)-W/2+min(W,H)*0.16,abs(Y-H/2)-H/2+min(W,H)*0.16),min(W,H)*0.16),0,255)"
    }
}

/// ffmpeg arguments that turn one segment into an mp4.
fn segment_args(root: &Path, seg: &project::Segment, size: (usize, usize), bubble: Option<Bubble>, max_width: usize, out: &Path) -> Result<Vec<String>> {
    let d0 = seg.display.start_time;
    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y"].map(String::from).to_vec();
    let mut input = |t: &Track| {
        args.extend(["-i".into(), root.join(&t.path).display().to_string()]);
    };
    input(&seg.display);
    let mut graph = String::from("[0:v]setpts=PTS-STARTPTS[base];");
    let mut v = "base";
    let mut n = 1;
    if let (Some(cam), Some(b)) = (&seg.camera, bubble) {
        input(cam);
        let (w, h) = (size.0 as f64, size.1 as f64);
        let (bw, bh) = (((b.w * w) as usize).max(16) & !1, ((b.h * h) as usize).max(16) & !1);
        let (bx, by) = ((b.x * w).round() as i64, (b.y * h).round() as i64);
        let flip = if b.mirror { "hflip," } else { "" };
        graph += &format!(
            "[1:v]{},crop='min(iw,ih*{r})':'min(ih,iw/{r})',scale={bw}:{bh},{flip}format=yuva420p,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='{}'[cam];[base][cam]overlay={bx}:{by}:eof_action=pass[withcam];",
            shift(cam.start_time - d0, false),
            bubble_alpha(b.round),
            r = bw as f64 / bh as f64,
        );
        v = "withcam";
        n += 1;
    }
    let mut audio = Vec::new();
    for t in [&seg.mic, &seg.system_audio].into_iter().flatten() {
        input(t);
        graph += &format!("[{n}:a]aresample=48000,aformat=channel_layouts=stereo,{}[a{n}];", shift(t.start_time - d0, true));
        audio.push(format!("[a{n}]"));
        n += 1;
    }
    // Cap's Instant resolution setting caps the width; never upscale.
    let scale = if size.0 > max_width { format!("scale={max_width}:-2,") } else { String::new() };
    graph += &format!("[{v}]{scale}format=yuv420p[v]");
    if audio.len() == 2 {
        graph += &format!(";{}amix=inputs=2:duration=longest:normalize=0[a]", audio.concat());
    } else if let [a] = &audio[..] {
        graph += &format!(";{a}anull[a]");
    }
    let duration = probe(&root.join(&seg.display.path), "duration")?;
    args.extend(["-filter_complex".into(), graph, "-map".into(), "[v]".into()]);
    if !audio.is_empty() {
        args.extend(["-map", "[a]", "-c:a", "aac", "-b:a", "160k"].map(String::from));
    }
    let video: &[&str] = if cfg!(target_os = "macos") { &["-c:v", "h264_videotoolbox", "-b:v", "10M"] } else { &["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"] };
    args.extend(video.iter().map(|s| s.to_string()));
    args.extend(["-t".into(), format!("{duration:.6}"), "-movflags".into(), "+faststart".into(), out.display().to_string()]);
    Ok(args)
}

fn ffmpeg(args: &[String]) -> Result<()> {
    let out = Command::new("ffmpeg").args(args).output().context("failed to start ffmpeg")?;
    if !out.status.success() {
        bail!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

/// Writes `content/output.mp4` (at most `max_width` wide) for a finished recording and returns its path.
pub fn finish(root: &Path, bubble: Option<Bubble>, max_width: usize) -> Result<PathBuf> {
    let meta: Meta = project::read(&root.join(project::META))?;
    let size = crate::export::probe_size(&root.join(&meta.segments.first().context("empty recording")?.display.path))?;
    let parts: Vec<PathBuf> = (0..meta.segments.len()).map(|k| root.join(format!("content/part-{k}.mp4"))).collect();
    for (seg, part) in meta.segments.iter().zip(&parts) {
        ffmpeg(&segment_args(root, seg, size, bubble, max_width, part)?)?;
    }
    let out = root.join(OUTPUT);
    if let [only] = &parts[..] {
        std::fs::rename(only, &out)?;
    } else {
        // Each pause made a new segment; join them without re-encoding. Names are relative to the
        // list, so ffmpeg's safe mode can stay on whatever the recording folder is called.
        let list = root.join("content/parts.txt");
        std::fs::write(&list, (0..parts.len()).map(|k| format!("file 'part-{k}.mp4'\n")).collect::<String>())?;
        let args = ["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-i", &list.display().to_string(), "-c", "copy", "-movflags", "+faststart", &out.display().to_string()];
        ffmpeg(&args.map(String::from))?;
        for p in parts.iter().chain([&list]) {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two synthetic sources with known offsets become one mp4 with the bubble baked in, and a
    /// second segment (a pause) is joined on, even in a folder whose name has a quote in it.
    #[test]
    fn bakes_camera_and_mixes_audio() {
        let root = std::env::temp_dir().join(format!("instant test's {}", std::process::id()));
        let seg = root.join("content/segments/segment-0");
        std::fs::create_dir_all(&seg).unwrap();
        std::fs::create_dir_all(root.join("content/segments/segment-1")).unwrap();
        let make = |args: &str, file: &str| {
            let mut a: Vec<String> = ["-v", "error", "-y"].map(String::from).to_vec();
            a.extend(args.split(' ').map(String::from));
            a.push(seg.join(file).display().to_string());
            ffmpeg(&a).unwrap();
        };
        make("-f lavfi -i color=c=blue:s=320x240:r=30:d=2 -c:v libx264 -pix_fmt yuv420p", "display.mkv");
        make("-f lavfi -i color=c=red:s=160x120:r=30:d=2 -c:v libx264 -pix_fmt yuv420p", "camera.mkv");
        make("-f lavfi -i sine=f=440:d=2 -c:a aac", "audio-input.mka");
        make("-f lavfi -i color=c=green:s=320x240:r=30:d=1 -c:v libx264 -pix_fmt yuv420p", "../segment-1/display.mkv");
        let track = |f: &str, t: f64| Track { path: format!("content/segments/segment-0/{f}"), fps: None, start_time: t };
        let meta = Meta {
            segments: vec![
                project::Segment { display: track("display.mkv", 0.0), camera: Some(track("camera.mkv", 0.2)), mic: Some(track("audio-input.mka", 0.1)), system_audio: None, cursor: None },
                project::Segment { display: track("../segment-1/display.mkv", 0.0), camera: None, mic: None, system_audio: None, cursor: None },
            ],
            ..Default::default()
        };
        project::write(&root.join(project::META), &meta).unwrap();
        let out = finish(&root, Some(Bubble { x: 0.5, y: 0.5, w: 0.25, h: 0.25, round: true, mirror: false }), 1920).unwrap();
        assert!((probe(&out, "duration").unwrap() - 3.0).abs() < 0.2);
        // The bubble's centre is red (camera), the corner is blue (screen).
        let frame = Command::new("ffmpeg").args(["-v", "error", "-ss", "1", "-i"]).arg(&out).args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]).output().unwrap().stdout;
        let px = |x: usize, y: usize| &frame[(y * 320 + x) * 3..(y * 320 + x) * 3 + 3];
        assert!(px(200, 150)[0] > 150 && px(200, 150)[2] < 100, "bubble centre {:?}", px(200, 150));
        assert!(px(10, 10)[2] > 150, "screen corner {:?}", px(10, 10));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
