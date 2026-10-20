//! Text recognition for selectable text in screenshots, with each OS's own engine:
//! Apple Vision on macOS, Windows.Media.Ocr on Windows, the `tesseract` CLI on Linux.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Debug, Clone)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One line of text; bounds in image pixels, top-left origin.
#[derive(Serialize, Debug, Clone)]
pub struct Line {
    pub text: String,
    pub confidence: f64,
    pub bounds: Bounds,
}

#[cfg(target_os = "macos")]
pub fn recognize(path: &Path) -> Result<Vec<Line>> {
    use anyhow::Context;
    use objc2::AnyThread;
    use objc2::rc::Retained;
    use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

    let (w, h) = image::image_dimensions(path)?;
    let (w, h) = (w as f64, h as f64);
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string()));
    let handler = unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    let as_request: Retained<VNRequest> = Retained::into_super(Retained::into_super(request.clone()));
    handler.performRequests_error(&NSArray::from_retained_slice(&[as_request])).map_err(|e| anyhow::anyhow!("text recognition failed: {e}"))?;
    let mut lines = Vec::new();
    for obs in request.results().context("no recognition results")?.iter() {
        let Some(best) = obs.topCandidates(1).firstObject() else { continue };
        // Vision's boxes are normalized with the origin at the bottom-left.
        let b = unsafe { obs.boundingBox() };
        lines.push(Line {
            text: best.string().to_string(),
            confidence: best.confidence() as f64,
            bounds: Bounds { x: b.origin.x * w, y: (1.0 - b.origin.y - b.size.height) * h, width: b.size.width * w, height: b.size.height * h },
        });
    }
    Ok(lines)
}

#[cfg(windows)]
pub fn recognize(path: &Path) -> Result<Vec<Line>> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::{FileAccessMode, StorageFile};
    use windows::core::HSTRING;

    // Not canonicalize: its \\?\ prefix isn't accepted by StorageFile.
    let absolute = std::path::absolute(path)?;
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(absolute.as_os_str()))?.join()?;
    let stream = file.OpenAsync(FileAccessMode::Read)?.join()?;
    let bitmap = BitmapDecoder::CreateAsync(&stream)?.join()?.GetSoftwareBitmapAsync()?.join()?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let mut lines = Vec::new();
    for line in engine.RecognizeAsync(&bitmap)?.join()?.Lines()? {
        // A line's box is the union of its words' boxes.
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for word in line.Words()? {
            let r = word.BoundingRect()?;
            (x0, y0) = (x0.min(r.X as f64), y0.min(r.Y as f64));
            (x1, y1) = (x1.max((r.X + r.Width) as f64), y1.max((r.Y + r.Height) as f64));
        }
        if x1 > x0 {
            // Windows gives no confidence score.
            lines.push(Line { text: line.Text()?.to_string(), confidence: 1.0, bounds: Bounds { x: x0, y: y0, width: x1 - x0, height: y1 - y0 } });
        }
    }
    Ok(lines)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn recognize(path: &Path) -> Result<Vec<Line>> {
    // shortcut: needs the tesseract CLI installed; without it there is simply no selectable text.
    let Ok(out) = std::process::Command::new("tesseract").arg(path).args(["-", "tsv"]).output() else { return Ok(Vec::new()) };
    Ok(parse_tsv(&String::from_utf8_lossy(&out.stdout)))
}

/// Groups tesseract's word rows (level 5) into lines by block/paragraph/line number.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn parse_tsv(tsv: &str) -> Vec<Line> {
    let mut lines: Vec<((u32, u32, u32), Line, f64, usize)> = Vec::new();
    for row in tsv.lines().skip(1) {
        let f: Vec<&str> = row.split('\t').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
        let (Some(5.0), Some(x), Some(y), Some(w), Some(h), Some(conf), Some(text)) = (num(0), num(6), num(7), num(8), num(9), num(10), f.get(11)) else { continue };
        if text.trim().is_empty() {
            continue;
        }
        let key = (num(2).unwrap_or(0.0) as u32, num(3).unwrap_or(0.0) as u32, num(4).unwrap_or(0.0) as u32);
        match lines.last_mut() {
            Some((k, line, sum, n)) if *k == key => {
                let b = &mut line.bounds;
                let (x1, y1) = ((b.x + b.width).max(x + w), (b.y + b.height).max(y + h));
                (b.x, b.y) = (b.x.min(x), b.y.min(y));
                (b.width, b.height) = (x1 - b.x, y1 - b.y);
                line.text = format!("{} {}", line.text, text.trim());
                (*sum, *n) = (*sum + conf, *n + 1);
            }
            _ => lines.push((key, Line { text: text.trim().into(), confidence: 0.0, bounds: Bounds { x, y, width: w, height: h } }, conf, 1)),
        }
    }
    lines.into_iter().map(|(_, mut l, sum, n)| { l.confidence = sum / n as f64 / 100.0; l }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tesseract_words_group_into_lines() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   4\t1\t1\t1\t1\t0\t10\t10\t200\t20\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t10\t80\t20\t90\tHello\n\
                   5\t1\t1\t1\t1\t2\t100\t12\t110\t18\t70\tworld\n\
                   5\t1\t1\t1\t2\t1\t10\t40\t50\t20\t80\tNext\n";
        let lines = parse_tsv(tsv);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!((lines[0].bounds.x, lines[0].bounds.width, lines[0].bounds.height), (10.0, 200.0, 20.0));
        assert!((lines[0].confidence - 0.8).abs() < 1e-9);
    }

    /// Reads known text back from a fixture (text drawn at x=40, y=80, 64 px); `--ignored`
    /// because it needs the OS engine (Linux: tesseract).
    #[test]
    #[ignore]
    fn live_recognizes_rendered_text() {
        let png = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/testdata/ocr.png");
        let lines = recognize(&png).unwrap();
        let line = lines.iter().find(|l| l.text.to_lowercase().contains("hello")).unwrap_or_else(|| panic!("got {lines:?}"));
        assert!((line.bounds.x - 40.0).abs() < 20.0 && line.bounds.y > 60.0 && line.bounds.y < 110.0, "{line:?}");
    }
}
