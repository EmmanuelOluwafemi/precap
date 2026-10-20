//! macOS screen capture through ScreenCaptureKit. Unlike ffmpeg's avfoundation input,
//! it can leave this app's own windows (controls bar, camera bubble) out of the
//! recording, which is how Cap does it. Frames go to ffmpeg's stdin as raw NV12.

use anyhow::{Context, Result};
use screencapturekit::cm::CMSampleBufferExt;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;
use screencapturekit::stream::configuration::color_matrix;
use std::io::Write;
use std::process::ChildStdin;
use std::sync::{Arc, Mutex, Weak};

struct Sink {
    stdin: Option<ChildStdin>,
    /// The last NV12 frame sent.
    frame: Vec<u8>,
}

struct Handler {
    sink: Arc<Mutex<Sink>>,
    w: usize,
    h: usize,
}

impl SCStreamOutputTrait for Handler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, kind: SCStreamOutputType) {
        if !matches!(kind, SCStreamOutputType::Screen) {
            return;
        }
        // Idle updates (nothing changed on screen) carry no image.
        let Some(pb) = sample.pixel_buffer() else { return };
        let Ok(g) = pb.lock(CVPixelBufferLockFlags::READ_ONLY) else { return };
        if g.width() != self.w || g.height() != self.h || g.plane_count() != 2 {
            return;
        }
        let mut sink = self.sink.lock().unwrap_or_else(|e| e.into_inner());
        sink.frame.clear();
        for (plane, rows) in [(0, self.h), (1, self.h / 2)] {
            let Some(data) = (unsafe { g.plane_data(plane) }) else { return };
            let stride = g.bytes_per_row_of_plane(plane);
            // Rows are padded to the stride; ffmpeg wants exactly `w` bytes per row.
            for y in 0..rows {
                sink.frame.extend_from_slice(&data[y * stride..y * stride + self.w]);
            }
        }
        let Sink { stdin, frame } = &mut *sink;
        if stdin.as_mut().is_some_and(|s| s.write_all(frame).is_err()) {
            *stdin = None;
        }
    }
}

/// Live sinks, so a quit or kill can close them too (ffmpeg only finalizes on EOF:
/// on a static screen it sits blocked reading and never sees SIGINT).
static OPEN: Mutex<Vec<Weak<Mutex<Sink>>>> = Mutex::new(Vec::new());

/// Frames only arrive when the screen changes; resend the last one so the video lasts
/// until now, then close stdin so ffmpeg finalizes the file.
fn finish(sink: &Mutex<Sink>) {
    let mut sink = sink.lock().unwrap_or_else(|e| e.into_inner());
    let Sink { stdin, frame } = &mut *sink;
    if let (Some(s), false) = (stdin.as_mut(), frame.is_empty()) {
        let _ = s.write_all(frame);
    }
    *stdin = None;
}

/// Closes every capture's pipe (used when the app is quit or killed).
pub fn close_all() {
    for sink in OPEN.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
        if let Some(sink) = sink.upgrade() {
            finish(&sink);
        }
    }
}

pub struct Capture {
    stream: SCStream,
    sink: Arc<Mutex<Sink>>,
}

/// What to capture: a whole display minus our own windows, or one window.
#[derive(Clone, Copy)]
pub enum Source {
    Display(u32),
    Window(u32),
}

/// `exclude`: other windows (by id) to leave out of a display capture, besides our own.
fn filter(content: &SCShareableContent, source: Source, exclude: &[u32]) -> Result<SCContentFilter> {
    let pid = std::process::id() as i32;
    Ok(match source {
        Source::Display(id) => {
            let display = content.displays().into_iter().find(|d| d.display_id() == id).context("display not found")?;
            let ours: Vec<SCWindow> =
                content.windows().into_iter().filter(|w| exclude.contains(&w.window_id()) || w.owning_application().is_some_and(|a| a.process_id() == pid)).collect();
            let ours: Vec<&SCWindow> = ours.iter().collect();
            SCContentFilter::create().with_display(&display).with_excluding_windows(&ours).build()?
        }
        Source::Window(id) => {
            let window = content.windows().into_iter().find(|w| w.window_id() == id).context("window not found (closed?)")?;
            SCContentFilter::create().with_window(&window).build()?
        }
    })
}

fn shareable() -> Result<SCShareableContent> {
    SCShareableContent::get().context("ScreenCaptureKit unavailable (Screen Recording permission?)")
}

/// Captures `source` at `w`x`h` pixels into `stdin` as NV12.
pub fn start(source: Source, w: usize, h: usize, fps: u32, show_cursor: bool, exclude: &[u32], stdin: ChildStdin) -> Result<Capture> {
    let filter = filter(&shareable()?, source, exclude)?;
    let config = SCStreamConfiguration::new()
        .with_width(w as u32)
        .with_height(h as u32)
        .with_pixel_format(PixelFormat::YCbCr_420v)
        // ffmpeg decodes untagged video as BT.601, so capture in it too.
        .with_color_matrix(color_matrix::ITU_R_601_4)?
        .with_shows_cursor(show_cursor)
        .with_minimum_frame_interval(&CMTime::new(1, fps as i32));
    let sink = Arc::new(Mutex::new(Sink { stdin: Some(stdin), frame: Vec::new() }));
    let mut stream = SCStream::new(&filter, &config)?;
    stream.add_output_handler(Handler { sink: sink.clone(), w, h }, SCStreamOutputType::Screen)?;
    stream.start_capture()?;
    OPEN.lock().unwrap_or_else(|e| e.into_inner()).push(Arc::downgrade(&sink));
    Ok(Capture { stream, sink })
}

impl Capture {
    /// Stops capturing and closes ffmpeg's stdin so it finalizes the file.
    pub fn stop(self) {
        let _ = self.stream.stop_capture();
        finish(&self.sink);
    }
}

/// Writes system audio as interleaved stereo f32 (ScreenCaptureKit hands over one plane per channel).
struct AudioHandler(Arc<Mutex<Sink>>);

impl SCStreamOutputTrait for AudioHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, kind: SCStreamOutputType) {
        if !matches!(kind, SCStreamOutputType::Audio) {
            return;
        }
        let Ok(list) = sample.audio_buffer_list() else { return };
        let planes: Vec<&[u8]> = (0..list.num_buffers()).filter_map(|i| list.get(i)).map(|b| b.data()).collect();
        let mut sink = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Sink { stdin, frame } = &mut *sink;
        frame.clear();
        match planes[..] {
            [l, r] if l.len() == r.len() => {
                for (a, b) in l.chunks_exact(4).zip(r.chunks_exact(4)) {
                    frame.extend_from_slice(a);
                    frame.extend_from_slice(b);
                }
            }
            [interleaved] => frame.extend_from_slice(interleaved),
            _ => return,
        }
        if stdin.as_mut().is_some_and(|s| s.write_all(frame).is_err()) {
            *stdin = None;
        }
    }
}

pub struct AudioCapture {
    stream: SCStream,
    sink: Arc<Mutex<Sink>>,
}

/// Captures what display `display_id`'s machine plays (48 kHz stereo f32le) into `stdin`, minus our own sounds.
pub fn start_audio(display_id: u32, stdin: ChildStdin) -> Result<AudioCapture> {
    let filter = filter(&shareable()?, Source::Display(display_id), &[])?;
    // Audio still needs a video stream; keep it tiny and slow.
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_minimum_frame_interval(&CMTime::new(1, 1))
        .with_captures_audio(true)
        .with_excludes_current_process_audio(true)
        .with_sample_rate(48000)
        .with_channel_count(2);
    let sink = Arc::new(Mutex::new(Sink { stdin: Some(stdin), frame: Vec::new() }));
    let mut stream = SCStream::new(&filter, &config)?;
    stream.add_output_handler(AudioHandler(sink.clone()), SCStreamOutputType::Audio)?;
    stream.start_capture()?;
    OPEN.lock().unwrap_or_else(|e| e.into_inner()).push(Arc::downgrade(&sink));
    Ok(AudioCapture { stream, sink })
}

impl AudioCapture {
    pub fn stop(self) {
        let _ = self.stream.stop_capture();
        // No frame to repeat: an audio "frame" is just the last chunk.
        self.sink.lock().unwrap_or_else(|e| e.into_inner()).frame.clear();
        finish(&self.sink);
    }
}

/// On-screen windows of other apps, front to back, in points.
pub fn windows() -> Result<Vec<crate::record::Window>> {
    let pid = std::process::id() as i32;
    Ok(shareable()?
        .windows()
        .into_iter()
        .filter(|w| w.is_on_screen() && w.window_layer() == 0)
        .filter_map(|w| {
            let app = w.owning_application().filter(|a| a.process_id() != pid)?;
            let f = w.frame();
            (f.size.width >= 50.0 && f.size.height >= 50.0).then(|| crate::record::Window {
                id: w.window_id(),
                title: w.title().unwrap_or_default(),
                app: app.application_name(),
                bundle: app.bundle_identifier(),
                x: f.origin.x,
                y: f.origin.y,
                w: f.size.width,
                h: f.size.height,
                display: 0,
            })
        })
        .collect())
}

/// Still image of `source` at `w`x`h` pixels; `rect` (points, in the display's own
/// coordinates) limits it to an area. Saved as PNG or JPEG by extension.
pub fn screenshot(source: Source, rect: Option<(f64, f64, f64, f64)>, w: usize, h: usize, exclude: &[u32], out: &std::path::Path) -> Result<()> {
    use screencapturekit::screenshot_manager::{CGImageExt, ImageFormat, SCScreenshotManager};
    let filter = filter(&shareable()?, source, exclude)?;
    let mut config = SCStreamConfiguration::new().with_width(w as u32).with_height(h as u32).with_shows_cursor(false);
    if let Some((x, y, rw, rh)) = rect {
        config = config.with_source_rect(screencapturekit::cg::CGRect::new(x, y, rw, rh));
    }
    let image = SCScreenshotManager::capture_image(&filter, &config).context("screenshot failed")?;
    let jpeg = out.extension().is_some_and(|e| e == "jpg");
    image.save(&out.display().to_string(), if jpeg { ImageFormat::Jpeg(0.8) } else { ImageFormat::Png }).context("saving screenshot")?;
    Ok(())
}
