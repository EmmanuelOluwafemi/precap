//! Auto-zoom: turn recorded mouse clicks into zoom segments on the timeline.
//! Cap has the same idea (`generate_zoom_segments_from_clicks` in its binary)
//! but its thresholds aren't visible from the app, so the timings here are ours.

use crate::project::{Config, CursorLog, Meta, ZoomMode, ZoomSegment};

/// Click-down times mapped through cuts and speed changes onto the output timeline (seconds, sorted).
pub fn click_times(meta: &Meta, cfg: &Config, cursors: &[CursorLog]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut offset = 0.0;
    for clip in &cfg.timeline.segments {
        let clicks = cursors.get(clip.recording_segment).map_or(&[][..], |c| &c.clicks[..]);
        for click in clicks.iter().filter(|c| c.down) {
            // Cursor times count from the segment start; the timeline counts display-file time.
            let s = click.time_ms / 1000.0 - meta.segments.get(clip.recording_segment).map_or(0.0, |seg| seg.display.start_time);
            if s >= clip.start && s < clip.end {
                out.push(offset + (s - clip.start) / clip.timescale);
            }
        }
        offset += (clip.end - clip.start) / clip.timescale;
    }
    out
}

/// Groups clicks into zoom segments that follow the cursor (`ZoomMode::Auto`).
///
/// `clicks`: sorted click times in seconds on the output timeline.
/// `duration`: total output length, so no segment runs past the end.
///
/// The zoom spring needs ~0.5 s to ease in and out, so segments shorter than
/// ~1 s feel twitchy, and two segments a fraction of a second apart read as a stutter.
/// `amount`: zoom level of each segment (Cap's "Default zoom amount" setting, 1.5 by default).
pub fn zoom_segments_from_clicks(clicks: &[f64], duration: f64, amount: f64) -> Vec<ZoomSegment> {
    // Start a little early so the zoom has landed by the click, hold a while after it,
    // and let clicks that close together share one zoom instead of stuttering in and out.
    const LEAD_IN: f64 = 0.4;
    const HOLD: f64 = 1.5;
    const MERGE_GAP: f64 = 1.0;
    const MIN_LEN: f64 = 1.0;
    let mut out: Vec<ZoomSegment> = Vec::new();
    for &t in clicks {
        let (start, end) = ((t - LEAD_IN).max(0.0), (t + HOLD).min(duration));
        match out.last_mut() {
            Some(last) if start - last.end < MERGE_GAP => last.end = last.end.max(end),
            _ => out.push(ZoomSegment { start, end, amount, mode: ZoomMode::Auto }),
        }
    }
    out.retain(|z| z.end - z.start >= MIN_LEN);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_become_merged_zooms() {
        let z = zoom_segments_from_clicks(&[2.0, 2.5, 3.0, 10.0, 19.9], 20.0, 1.5);
        let spans: Vec<(f64, f64)> = z.iter().map(|z| (z.start, z.end)).collect();
        // 2.0-3.0 merge into one zoom; 10.0 is its own; 19.9 is too close to the end to be worth a zoom.
        assert_eq!(spans, vec![(1.6, 4.5), (9.6, 11.5)]);
        assert!(z.iter().all(|z| z.mode == ZoomMode::Auto));
        assert!(zoom_segments_from_clicks(&[], 5.0, 1.5).is_empty());
    }
}
