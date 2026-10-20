fn main() {
    // Tauri embeds ui/ at compile time; rebuild when the front end changes.
    println!("cargo:rerun-if-changed=ui");
    // The ScreenCaptureKit bindings ship a Swift bridge; point the loader at the system Swift runtime.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        // The bridges also need the toolchain's static Swift compatibility libraries; with only the
        // Command Line Tools installed, the crates' own guess at their folder doesn't exist.
        if let Ok(out) = std::process::Command::new("xcrun").args(["--find", "swiftc"]).output() {
            let swiftc = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            if let Some(usr) = swiftc.parent().and_then(|bin| bin.parent()) {
                println!("cargo:rustc-link-search={}", usr.join("lib/swift/macosx").display());
            }
        }
    }
    tauri_build::build()
}
