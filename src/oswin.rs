//! Listing other apps' windows on Windows and Linux (macOS uses ScreenCaptureKit, see sck.rs).

use crate::record::Window;

/// Visible top-level windows with a title, topmost first, in screen pixels.
#[cfg(windows)]
pub fn windows() -> Vec<Window> {
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible};

    unsafe extern "system" fn each(hwnd: HWND, out: LPARAM) -> i32 {
        unsafe {
            let out = &mut *(out as *mut Vec<Window>);
            let mut cloaked = 0u32;
            DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as _, 4);
            if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 || cloaked != 0 {
                return 1;
            }
            let mut title = [0u16; 512];
            let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut r: RECT = std::mem::zeroed();
            if n <= 0 || pid == std::process::id() || GetWindowRect(hwnd, &mut r) == 0 {
                return 1;
            }
            // The app name is its executable's file stem.
            let mut app = String::new();
            let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !proc.is_null() {
                let (mut path, mut len) = ([0u16; 1024], 1024u32);
                if QueryFullProcessImageNameW(proc, 0, path.as_mut_ptr(), &mut len) != 0 {
                    let path = String::from_utf16_lossy(&path[..len as usize]);
                    app = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                }
                CloseHandle(proc);
            }
            let (w, h) = ((r.right - r.left) as f64, (r.bottom - r.top) as f64);
            if w >= 50.0 && h >= 50.0 {
                // Window handles fit in 32 bits (they are shared with 32-bit processes).
                let title = String::from_utf16_lossy(&title[..n as usize]);
                out.push(Window { id: hwnd as usize as u32, title, app, bundle: String::new(), x: r.left as f64, y: r.top as f64, w, h, display: 0 });
            }
            1
        }
    }
    let mut out: Vec<Window> = Vec::new();
    // EnumWindows walks top-level windows in z-order, topmost first.
    unsafe { EnumWindows(Some(each), &mut out as *mut Vec<Window> as LPARAM) };
    out
}

/// Managed windows from the window manager's stacking list, topmost first (X11 only).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn windows() -> Vec<Window> {
    list_x11().unwrap_or_default()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn list_x11() -> Option<Vec<Window>> {
    use xcb::{Xid, XidNew, x};
    let (conn, screen) = xcb::Connection::connect(None).ok()?;
    let root = conn.get_setup().roots().nth(screen as usize)?.root();
    let atom = |name: &str| conn.wait_for_reply(conn.send_request(&x::InternAtom { only_if_exists: true, name: name.as_bytes() })).ok().map(|r| r.atom());
    let prop = |w: x::Window, property: x::Atom, r#type: x::Atom| {
        conn.wait_for_reply(conn.send_request(&x::GetProperty { delete: false, window: w, property, r#type, long_offset: 0, long_length: 4096 })).ok()
    };
    let (stacking, name, utf8, pid_atom) = (atom("_NET_CLIENT_LIST_STACKING")?, atom("_NET_WM_NAME")?, atom("UTF8_STRING")?, atom("_NET_WM_PID")?);
    let ids: Vec<u32> = prop(root, stacking, x::ATOM_WINDOW)?.value::<u32>().to_vec();
    let mut out = Vec::new();
    // The stacking list runs bottom to top.
    for id in ids.into_iter().rev() {
        let w = x::Window::new(id);
        let pid = prop(w, pid_atom, x::ATOM_CARDINAL).and_then(|r| r.value::<u32>().first().copied());
        if pid == Some(std::process::id()) {
            continue;
        }
        let Some(geo) = conn.wait_for_reply(conn.send_request(&x::GetGeometry { drawable: x::Drawable::Window(w) })).ok() else { continue };
        let Some(pos) = conn.wait_for_reply(conn.send_request(&x::TranslateCoordinates { src_window: w, dst_window: root, src_x: 0, src_y: 0 })).ok() else { continue };
        let text = |p: Option<x::GetPropertyReply>| p.map(|r| String::from_utf8_lossy(r.value::<u8>()).into_owned()).unwrap_or_default();
        let mut title = text(prop(w, name, utf8));
        if title.is_empty() {
            title = text(prop(w, x::ATOM_WM_NAME, x::ATOM_STRING));
        }
        // WM_CLASS is "instance\0Class\0".
        let class = text(prop(w, x::ATOM_WM_CLASS, x::ATOM_STRING));
        let app = class.split('\0').filter(|s| !s.is_empty()).last().unwrap_or("").to_string();
        let (ww, wh) = (geo.width() as f64, geo.height() as f64);
        if ww >= 50.0 && wh >= 50.0 && !title.is_empty() {
            out.push(Window { id: w.resource_id(), title, app, bundle: String::new(), x: pos.dst_x() as f64, y: pos.dst_y() as f64, w: ww, h: wh, display: 0 });
        }
    }
    Some(out)
}
