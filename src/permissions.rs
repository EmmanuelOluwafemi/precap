//! First-run permission onboarding. macOS silently records black frames (or no cursor track)
//! when privacy permissions are missing, so a small window lists what Precap needs and links
//! to the right System Settings pane. Other OSes have no such prompts and skip all of this.

use crate::{recorder, settings};
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Granted,
    NotDetermined,
    Denied,
}

#[derive(Serialize, Clone, Debug)]
pub struct Perm {
    pub id: &'static str,
    pub required: bool,
    pub status: Status,
}

/// Onboarding shows on macOS until completed, and again whenever a required permission goes missing.
pub fn needed(done: bool, macos: bool, perms: &[Perm]) -> bool {
    macos && (!done || perms.iter().any(|p| p.required && p.status != Status::Granted))
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Perm, Status};
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, Bool};
    use objc2_foundation::NSString;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    // Only for the AVCaptureDevice class, which is looked up through the runtime.
    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {}

    // AVMediaTypeAudio / AVMediaTypeVideo.
    const MIC: &str = "soun";
    const CAM: &str = "vide";

    fn av_status(media: &str) -> Status {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else { return Status::NotDetermined };
        let ty = NSString::from_str(media);
        let s: isize = unsafe { msg_send![cls, authorizationStatusForMediaType: &*ty] };
        match s {
            3 => Status::Granted,
            0 => Status::NotDetermined,
            _ => Status::Denied, // restricted or denied
        }
    }

    pub fn statuses() -> Vec<Perm> {
        let b = |ok: bool| if ok { Status::Granted } else { Status::Denied };
        vec![
            Perm { id: "screen", required: true, status: b(unsafe { CGPreflightScreenCaptureAccess() }) },
            Perm { id: "microphone", required: false, status: av_status(MIC) },
            Perm { id: "camera", required: false, status: av_status(CAM) },
            Perm { id: "accessibility", required: false, status: b(unsafe { AXIsProcessTrusted() }) },
        ]
    }

    /// Shows the system prompt where an API exists. False when the id has none.
    pub fn request(id: &str) -> bool {
        let media = match id {
            "screen" => {
                unsafe { CGRequestScreenCaptureAccess() };
                return true;
            }
            "microphone" => MIC,
            "camera" => CAM,
            _ => return false,
        };
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else { return false };
        let ty = NSString::from_str(media);
        let done = block2::RcBlock::new(|_granted: Bool| {});
        let _: () = unsafe { msg_send![cls, requestAccessForMediaType: &*ty, completionHandler: &*done] };
        true
    }
}

fn statuses() -> Vec<Perm> {
    #[cfg(target_os = "macos")]
    return mac::statuses();
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

#[tauri::command]
pub fn permission_status() -> Vec<Perm> {
    statuses()
}

/// The system prompt for `id`, or its System Settings pane when there is no prompt API.
#[tauri::command]
pub fn permission_request(id: String) {
    #[cfg(target_os = "macos")]
    if mac::request(&id) {
        return;
    }
    open_permission_pane(id);
}

#[tauri::command]
pub fn open_permission_pane(id: String) {
    let _pane = match id.as_str() {
        "screen" => "Privacy_ScreenCapture",
        "microphone" => "Privacy_Microphone",
        "camera" => "Privacy_Camera",
        "accessibility" => "Privacy_Accessibility",
        _ => return,
    };
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(format!("x-apple.systempreferences:com.apple.preference.security?{_pane}")).spawn();
}

#[tauri::command]
pub fn relaunch_app(app: AppHandle) {
    app.restart();
}

/// Shows the onboarding window (or focuses it).
#[tauri::command]
pub fn open_onboarding(app: AppHandle) -> Result<(), String> {
    window(&app).map_err(|e| e.to_string())
}

fn window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("onboarding") {
        w.show()?;
        return w.set_focus();
    }
    let b = WebviewWindowBuilder::new(app, "onboarding", WebviewUrl::App("onboarding.html".into()))
        .title("Precap Permissions")
        .inner_size(460.0, 560.0)
        .resizable(false)
        .maximizable(false)
        .theme(Some(tauri::Theme::Dark));
    #[cfg(target_os = "macos")]
    let b = b.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    b.build()?.set_focus()
}

/// Marks onboarding done, closes it and opens the recorder window.
#[tauri::command]
pub fn finish_onboarding(app: AppHandle, st: tauri::State<recorder::RecState>) -> Result<(), String> {
    recorder::set_settings(app.clone(), st, serde_json::json!({ "onboardingDone": true }))?;
    recorder::main_window(&app).map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window("onboarding") {
        let _ = w.close();
    }
    Ok(())
}

/// Startup hook: true when the onboarding window takes the place of the recorder window.
pub fn startup(app: &AppHandle, s: &settings::Settings) -> tauri::Result<bool> {
    if !cfg!(target_os = "macos") {
        if !s.onboarding_done {
            let _ = settings::save(&app.path().app_data_dir()?, &settings::Settings { onboarding_done: true, ..s.clone() });
        }
        return Ok(false);
    }
    let show = needed(s.onboarding_done, true, &statuses());
    if show {
        window(app)?;
    }
    Ok(show)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perms(screen: Status) -> Vec<Perm> {
        vec![Perm { id: "screen", required: true, status: screen }, Perm { id: "camera", required: false, status: Status::Denied }]
    }

    #[test]
    fn onboarding_decision() {
        assert!(needed(false, true, &perms(Status::Granted)), "first run");
        assert!(needed(true, true, &perms(Status::Denied)), "required permission revoked");
        assert!(!needed(true, true, &perms(Status::Granted)), "optional ones never block");
        assert!(!needed(false, false, &[]), "no TCC off macOS");
    }

    /// Status APIs only, never prompts. Run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    #[cfg(target_os = "macos")]
    fn prints_statuses() {
        println!("{:?}", statuses());
    }
}
