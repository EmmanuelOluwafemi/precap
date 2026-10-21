//! App settings, saved as `settings.json` in the app data folder (Cap's `general_settings`
//! key names, flattened). Read by the recorder app and the editors.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DEFAULT_NAME_TEMPLATE: &str = "{target_name} ({target_kind}) {date} {time}";

/// A key chord, recorded from a browser KeyboardEvent (`code` like "KeyR").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Hotkey {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self { code: String::new(), ctrl: false, shift: false, alt: false, meta: false }
    }
}

/// Cap's excluded-window rule: match by bundle id, else by app name (and title if given), else by title.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowExclusion {
    pub bundle_identifier: Option<String>,
    pub owner_name: Option<String>,
    pub window_title: Option<String>,
}

impl WindowExclusion {
    pub fn matches(&self, w: &crate::record::Window) -> bool {
        match (&self.bundle_identifier, &self.owner_name, &self.window_title) {
            (Some(b), ..) if !b.is_empty() => *b == w.bundle,
            (_, Some(o), title) => *o == w.app && title.as_ref().is_none_or(|t| *t == w.title),
            (_, None, Some(t)) => *t == w.title,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub recording_countdown: u32,
    pub confirm_before_recording_without_microphone: bool,
    /// "close" (hide) or "minimise".
    pub main_window_recording_start_behaviour: String,
    /// "openEditor" or "showOverlay".
    pub post_studio_recording_behaviour: String,
    /// "edit" (open the editor), "copy" (clipboard only) or "copyAndEdit".
    pub post_screenshot_behaviour: String,
    pub auto_zoom_on_clicks: bool,
    pub default_zoom_amount: f64,
    /// shortcut: Cap defaults to 60, but 60 fps of raw retina frames through a pipe is heavy, so 30.
    pub max_fps: u32,
    pub recordings_path: Option<String>,
    pub default_project_name_template: Option<String>,
    pub excluded_windows: Vec<WindowExclusion>,
    /// "balanced", "compatibility" (smaller files) or "ultra".
    pub studio_recording_quality: String,
    /// Max output width of Instant videos.
    pub instant_mode_max_resolution: u32,
    pub hotkeys: BTreeMap<String, Hotkey>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recording_countdown: 3,
            confirm_before_recording_without_microphone: true,
            main_window_recording_start_behaviour: "close".into(),
            post_studio_recording_behaviour: "openEditor".into(),
            post_screenshot_behaviour: "edit".into(),
            auto_zoom_on_clicks: false,
            default_zoom_amount: 1.5,
            max_fps: 30,
            recordings_path: None,
            default_project_name_template: None,
            excluded_windows: Vec::new(),
            studio_recording_quality: "balanced".into(),
            instant_mode_max_resolution: 1920,
            hotkeys: BTreeMap::new(),
        }
    }
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

/// Saved settings, or defaults when missing or unreadable.
pub fn load(data_dir: &Path) -> Settings {
    crate::project::read(&path(data_dir)).unwrap_or_default()
}

/// Merges `patch` (a partial settings object) into `current`.
pub fn merged(current: &Settings, patch: Value) -> Result<Settings> {
    let mut v = serde_json::to_value(current)?;
    if let (Value::Object(base), Value::Object(edits)) = (&mut v, patch) {
        base.extend(edits);
    }
    Ok(serde_json::from_value(v)?)
}

pub fn save(data_dir: &Path, s: &Settings) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    crate::project::write(&path(data_dir), s)
}

/// Fills Cap's project name tokens: {target_name} {target_kind} {mode} {date} {time} and
/// {date:FMT} {time:FMT} {moment:FMT} with strftime formats.
pub fn format_name(template: &str, target_name: &str, target_kind: &str, mode: &str, now: chrono::DateTime<chrono::Local>) -> Result<String> {
    use std::fmt::Write;
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else { break };
        let token = &rest[start + 1..start + end];
        let (name, fmt) = token.split_once(':').map_or((token, None), |(n, f)| (n, Some(f)));
        let mut fmt_now = |f: &str| write!(out, "{}", now.format(f)).map_err(|_| anyhow::anyhow!("bad date format \"{f}\""));
        match (name, fmt) {
            ("target_name", None) => out.push_str(target_name),
            ("target_kind", None) => out.push_str(target_kind),
            ("mode", None) => out.push_str(mode),
            ("date", None) => fmt_now("%Y-%m-%d")?,
            ("time", None) => fmt_now("%I.%M %p")?,
            ("date" | "time" | "moment", Some(f)) => fmt_now(f)?,
            _ => out.push_str(&rest[start..start + end + 1]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    Ok(crate::os::file_name(&out).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn names_follow_caps_template() {
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 20, 15, 5, 0).unwrap();
        let name = format_name(DEFAULT_NAME_TEMPLATE, "Built-in Retina Display", "Display", "Studio", now).unwrap();
        assert_eq!(name, "Built-in Retina Display (Display) 2026-10-20 03.05 PM");
        assert_eq!(format_name("{mode} {date:%d.%m} {unknown}", "x", "y", "Instant", now).unwrap(), "Instant 20.10 {unknown}");
        // Window titles can hold anything; folder names must not.
        assert_eq!(format_name("{target_name}", "Inbox | Mail\nfile '/etc/x'", "", "", now).unwrap(), "Inbox - Mail-file '-etc-x'");
    }

    #[test]
    fn patches_merge_and_exclusions_match() {
        let s = merged(&Settings::default(), serde_json::json!({"maxFps": 60, "hotkeys": {"stopRecording": {"code": "KeyS", "meta": true}}})).unwrap();
        assert_eq!((s.max_fps, s.recording_countdown, s.hotkeys["stopRecording"].meta), (60, 3, true));
        let w = crate::record::Window { id: 1, title: "Inbox".into(), app: "Mail".into(), bundle: "com.apple.mail".into(), x: 0.0, y: 0.0, w: 100.0, h: 100.0, display: 0 };
        let rule = |b: Option<&str>, o: Option<&str>, t: Option<&str>| WindowExclusion { bundle_identifier: b.map(Into::into), owner_name: o.map(Into::into), window_title: t.map(Into::into) };
        assert!(rule(Some("com.apple.mail"), None, None).matches(&w));
        assert!(rule(None, Some("Mail"), None).matches(&w));
        assert!(!rule(None, Some("Mail"), Some("Drafts")).matches(&w));
        assert!(rule(None, None, Some("Inbox")).matches(&w));
    }
}
