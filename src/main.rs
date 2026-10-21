mod autozoom;
mod editor;
mod export;
mod instant;
mod ocr;
mod os;
mod permissions;
mod project;
mod record;
mod recorder;
mod render;
mod screenshot;
mod segment;
mod settings;
#[cfg(not(target_os = "macos"))]
mod oswin;
#[cfg(target_os = "macos")]
mod sck;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Studio-style screen recorder: record screen + camera + mic, edit, export a
/// polished video. Run without a command to open the recorder window.
/// Needs ffmpeg and ffprobe on PATH.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// List capture devices (screens, cameras, microphones) via ffmpeg
    Devices,
    /// Record. Enter pauses/resumes (each pause starts a new segment), q + Enter stops
    Record {
        /// Project folder to create [default: recording-<unix time>.cap]
        out: Option<PathBuf>,
        /// Screen to capture (number from `devices`)
        #[arg(long, default_value_t = 0)]
        display: usize,
        /// Also record a camera; optionally name the device
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        camera: Option<String>,
        /// Also record a microphone; optionally name the device
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        mic: Option<String>,
        #[arg(long, default_value_t = 30)]
        fps: u32,
    },
    /// Render the project (project-config.json) to an mp4
    Export {
        project: PathBuf,
        /// A .gif extension exports a GIF [default: <project name>.mp4 in the current folder]
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        #[arg(long, default_value_t = 1080)]
        height: usize,
    },
    /// Render one frame to PNG, to check edits without a full export
    Preview {
        project: PathBuf,
        /// Seconds on the output timeline
        #[arg(long, default_value_t = 0.0)]
        at: f64,
        #[arg(long, default_value = "preview.png")]
        out: PathBuf,
        #[arg(long, default_value_t = 1080)]
        height: usize,
    },
    /// Open the visual editor with live preview
    Edit { project: PathBuf },
    /// Open the screenshot editor on a screenshot project (<name>.cap with original.png)
    Screenshot { project: PathBuf },
    /// Add zoom segments around recorded clicks to project-config.json
    AutoZoom { project: PathBuf },
}

fn main() -> Result<()> {
    // A quit or kill (Ctrl-C, SIGTERM, SIGHUP) must not leave ffmpeg capturing in the background.
    ctrlc::set_handler(|| {
        record::stop_all_children();
        std::process::exit(130);
    })?;
    let Some(cmd) = Cli::parse().cmd else { return recorder::run() };
    match cmd {
        Cmd::Devices => record::list_devices()?,
        Cmd::Record { out, display, camera, mic, fps } => {
            let device = |v: Option<String>, is_camera| match v.as_deref() {
                Some("") => record::default_device(is_camera).map(Some),
                _ => Ok(v),
            };
            let src = record::Sources { display, area: None, window: None, camera: device(camera, true)?, mic: device(mic, false)?, system_audio: false, fps, camera_only: false, show_cursor: false, exclude: vec![], quality: "balanced".into() };
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
            let out = out.unwrap_or_else(|| PathBuf::from(format!("recording-{secs}.cap")));
            let name = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            record::record(&out, &name, src)?;
            println!("edit {} then run: precap export {}", out.join(project::CONFIG).display(), out.display());
        }
        Cmd::Export { project, out, fps, height } => {
            let name = project.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or("export".into());
            let out = out.unwrap_or_else(|| PathBuf::from(format!("{name}.mp4")));
            let mut last = 0;
            let p = export::Project::load(&project)?;
            export::render(&p, fps, height, export::target(out.clone(), 20), &mut |done, total| {
                if done * 100 / total != last {
                    last = done * 100 / total;
                    eprint!("\rexporting {last:>3}%");
                }
            })?;
            eprintln!("\rexported {}", out.display());
        }
        Cmd::Preview { project, at, out, height } => {
            export::render(&export::Project::load(&project)?, 30, height, export::Target::Still(at, out), &mut |_, _| {})?
        }
        Cmd::Edit { project } => editor::run(&project)?,
        Cmd::Screenshot { project } => screenshot::run(&project)?,
        Cmd::AutoZoom { project } => {
            let p = export::Project::load(&project)?;
            let duration = p.cfg.timeline.segments.iter().map(|c| (c.end - c.start) / c.timescale).sum();
            let clicks = autozoom::click_times(&p.meta, &p.cfg, &p.cursors);
            let mut cfg = p.cfg;
            cfg.timeline.zoom_segments = autozoom::zoom_segments_from_clicks(&clicks, duration, 1.5);
            println!("{} clicks → {} zoom segments", clicks.len(), cfg.timeline.zoom_segments.len());
            project::write(&project.join(project::CONFIG), &cfg)?;
        }
    }
    Ok(())
}
