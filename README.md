# Precap

A studio-style screen recorder in Rust, modelled on how Cap's "studio mode" works.
You record raw tracks, edit a JSON project, and export a polished video.

- **Record:** a display, an area or a single window, plus optional camera, mic and system audio, plus cursor movement and clicks. Each pause starts a new segment.
- **Modes:** Studio (edit afterwards), Instant (a finished MP4 the moment you stop, copied to the clipboard; local only, no upload) and Screenshot (capture, then annotate; text in screenshots can be selected and copied).
- **Settings and shortcuts:** countdown, frame rate, recording quality, Instant resolution, recordings folder, project name template, auto zoom, windows to leave out of recordings (macOS), and global keyboard shortcuts for every recording and screenshot action.
- **Edit:** `project-config.json` controls background, padding, squircle corners, shadow, camera bubble, cursor smoothing, cuts, speed and zoom.
- **Export:** an mp4 with a spring-smoothed cursor, zooms that follow the cursor, and denoised, loudness-normalized audio.

It also exports studio projects recorded by Cap itself, because it reads the same
`recording-meta.json`, `cursor.json` and `project-config.json` format.

## Requirements

`ffmpeg` and `ffprobe` must be on your PATH. They handle capture, decode and encode on every OS.

| OS | Screen / window | Camera / mic | System audio | Extra |
|----|--------|--------------|--------------|-------|
| macOS 14+ | ScreenCaptureKit | avfoundation | ScreenCaptureKit | Grant Screen Recording, Camera, Microphone and **Accessibility** (Accessibility is for cursor tracking) |
| Windows | gdigrab | dshow (pass device names) | a loopback input such as "Stereo Mix" or VB-CABLE | none |
| Linux | x11grab (X11 only, no Wayland) | v4l2 / PulseAudio | PulseAudio/PipeWire monitor | `apt install pkg-config libx11-dev libxcb1-dev libxcb-randr0-dev libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev`; `xclip` (or `wl-copy`) for copying to the clipboard; `tesseract-ocr` for selectable text in screenshots |

The recording controls bar, camera bubble and teleprompter are kept out of the recorded video on macOS,
where ScreenCaptureKit excludes the app's own windows, as Cap does. On Windows they are marked
excluded from capture, but that is untested. On Linux, X11 capture can't exclude windows, so they
will appear in the recording.

The editor window uses the system webview: WKWebView on macOS, WebView2 on Windows (preinstalled on 10/11), and WebKitGTK on Linux.

## Use

```sh
cargo build --release
B=target/release/precap

$B                                            # recorder window: Instant / Studio / Screenshot modes
$B screenshot "My shot.cap"                   # screenshot editor (annotations, background, padding)
$B devices                                    # list screens/cameras/mics
$B record demo.cap --display 0 --camera --mic # terminal recorder: Enter = pause/resume, q + Enter = stop
$B edit demo.cap                              # visual editor with live preview
$B preview demo.cap --at 5 --out frame.png    # check an edit without exporting
$B auto-zoom demo.cap                         # add zooms around clicks (0.4 s lead-in, 1.5 s hold, clicks < 1 s apart merge)
$B export demo.cap --out demo.mp4 --height 1080 --fps 30
```

## Editing `project-config.json`

```jsonc
{
  "aspectRatio": null,              // or "wide" | "vertical" | "square" | "classic"
  "background": {
    "source": { "type": "gradient", "from": [71,59,214], "to": [233,119,163], "angle": 135 },
    // or { "type": "color", "value": [r,g,b] } / { "type": "image", "path": "/abs/file.jpg" }
    "padding": 8, "rounding": 8, "roundingType": "squircle",
    "shadow": 60, "advancedShadow": { "size": 50, "opacity": 18, "blur": 50 }
  },
  "camera": { "hide": false, "mirror": false, "position": { "x": "right", "y": "bottom" },
              "manualPosition": null, "size": 25, "rounding": 100 },
  "cursor": { "hide": false, "size": 100, "raw": false, "tension": 470, "mass": 3, "friction": 70 },
  "audio": { "mute": false, "improve": true, "micVolumeDb": 0, "systemVolumeDb": 0 }, // -30 dB or lower = muted
  "screenMovementSpring": { "stiffness": 200, "damping": 40, "mass": 2.25 },
  "timeline": {
    "segments": [ { "recordingSegment": 0, "timescale": 1.0, "start": 0, "end": 12.5 } ],
    "zoomSegments": [ { "start": 3, "end": 6, "amount": 2.0, "mode": "auto" } ]
  }
}
```

`timeline.segments` is the cut list. It plays slices of recording segments in order,
and `timescale: 2` doubles the speed. Zoom `mode` can be `"auto"` (follow the cursor)
or `{ "manual": { "x": 0.3, "y": 0.6 } }`.

Recordings made from the recorder window are saved in the app data folder, for example
`~/Library/Application Support/dev.precap.editor/recordings` on macOS. When you stop,
the editor opens on the new recording.

Settings are saved as `settings.json` in the app data folder (Cap's setting names).

The built-in wallpapers in `ui/wallpapers/` are original images generated with OpenAI's image
API by `tools/gen-wallpapers.py` (it reads `OPENAI_API_KEY`, or `~/.config/precap/openai.env`,
and only generates what's missing).

Screenshots live in the same app data folder under `screenshots/`, each a `<name>.cap`
folder with `original.png`, `recording-meta.json` and `project-config.json` (annotations are
kept in the config). Saved PNGs go to your Pictures folder; Instant exports go to Movies/Videos.

## Not included (Cap has these)

- Cap accounts, cloud upload and share links (Instant mode here is local only); the account,
  integrations, transcription, automations and changelog pages of Cap's settings
- Camera background blur
- Captions and transcription
- Keyboard overlay
- Text, mask and scene tracks in the video editor
- Motion blur
- GIF export
- Cap's own bundled wallpapers (ours are original)
- Multi-monitor cursor mapping
