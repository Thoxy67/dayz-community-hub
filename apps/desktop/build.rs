//! Windows only: embed the app icon, the version information (VERSIONINFO) and the
//! application manifest into the executable, as the Tauri build did (`tauri-build` with
//! `apps/gui/src-tauri/windows/app.manifest`). Explorer, the taskbar, Alt+Tab and the
//! `dzch://` / `.dzch` registrations (`platform::register`, `"<exe>",0`) show icon 0.
//!
//! On every other target this does nothing. A missing resource compiler (`llvm-rc` when
//! cross-compiling with cargo-xwin, `rc.exe` on Windows) is a warning, so a check still
//! links, unless `DZCH_REQUIRE_WINRES=1` (for release builds) makes it an error.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/app/icon.ico");
    println!("cargo:rerun-if-changed=windows/app.manifest");
    println!("cargo:rerun-if-env-changed=DZCH_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let bin = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
    let mut res = winresource::WindowsResource::new();
    res.set_icon(&format!("{dir}/assets/app/icon.ico"))
        .set_manifest_file(&format!("{dir}/windows/app.manifest"))
        .set("ProductName", "DayZ Community Hub")
        .set("FileDescription", "DayZ Community Hub")
        .set("CompanyName", "Thoxy")
        .set("LegalCopyright", "Copyright (c) 2026 Thoxy. MIT License.")
        .set("OriginalFilename", &format!("{bin}.exe"))
        .set("InternalName", &bin);
    // Linked into the executable only, not the examples or tests.
    if let Err(e) = res.compile_for(&bin) {
        if std::env::var_os("DZCH_REQUIRE_WINRES").is_some() {
            eprintln!("embedding the Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning={bin}.exe built without icon, version and manifest: {e}");
    }
}
