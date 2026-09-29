//! Embeds Valve's Steamworks redistributable for the target into the crate.
//!
//! Nothing binary is committed: the library comes from the sources of the
//! `steamworks-sys` crate (a build-dependency, so cargo has downloaded it by
//! the time this runs), which ship `lib/steam/redistributable_bin/<platform>/`.
//! It is looked for, in order:
//!
//! 1. `$DZ_STEAMWORKS_SDK`: an SDK's `sdk` directory (the one holding
//!    `redistributable_bin`), to use another SDK by hand;
//! 2. cargo's registry sources, `$CARGO_HOME/registry/src/*/steamworks-sys-<v>`;
//! 3. `cargo metadata --offline`, which also knows vendored sources.
//!
//! Only x86_64 Linux and Windows are embedded; elsewhere the Steamworks mode
//! reports itself unavailable. The Windows DLL is chosen by the *target*, so
//! the cross build (cargo-xwin, on Linux) embeds the right one.

use std::env;
use std::path::{Path, PathBuf};

/// The pinned `steamworks-sys` (see the workspace's Cargo.toml).
const SYS_VERSION: &str = "0.13.0";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=DZ_STEAMWORKS_SDK");
    println!("cargo::rustc-check-cfg=cfg(dz_steamworks_redist)");

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let (platform, file) = match (os.as_str(), arch.as_str()) {
        ("linux", "x86_64") => ("linux64", "libsteam_api.so"),
        ("windows", "x86_64") => ("win64", "steam_api64.dll"),
        _ => return,
    };

    let sdk = find_sdk().unwrap_or_else(|| {
        panic!(
            "Valve's Steamworks redistributable was not found: steamworks-sys {SYS_VERSION} \
             is not in cargo's registry sources, and `cargo metadata` did not name it. \
             Run `cargo fetch`, or set DZ_STEAMWORKS_SDK to an SDK's `sdk` directory."
        )
    });
    let src = sdk.join("redistributable_bin").join(platform).join(file);
    if !src.is_file() {
        panic!("{} is missing from the Steamworks SDK", src.display());
    }
    println!("cargo::rerun-if-changed={}", src.display());

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join(file);
    std::fs::copy(&src, &out).unwrap_or_else(|e| panic!("copying {} failed: {e}", src.display()));
    println!("cargo::rustc-cfg=dz_steamworks_redist");
    println!("cargo::rustc-env=DZ_STEAMWORKS_REDIST={}", out.display());
    println!("cargo::rustc-env=DZ_STEAMWORKS_REDIST_NAME={file}");
}

/// The SDK directory holding `redistributable_bin`.
fn find_sdk() -> Option<PathBuf> {
    let has_bins = |p: &Path| p.join("redistributable_bin").is_dir();
    if let Some(dir) = env::var_os("DZ_STEAMWORKS_SDK").map(PathBuf::from) {
        if has_bins(&dir) {
            return Some(dir);
        }
        panic!(
            "DZ_STEAMWORKS_SDK={} holds no redistributable_bin",
            dir.display()
        );
    }
    from_registry()
        .or_else(from_metadata)
        .filter(|p| has_bins(p))
}

fn cargo_home() -> Option<PathBuf> {
    env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| {
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(|h| PathBuf::from(h).join(".cargo"))
    })
}

/// `$CARGO_HOME/registry/src/<index>/steamworks-sys-<v>/lib/steam`.
fn from_registry() -> Option<PathBuf> {
    let src = cargo_home()?.join("registry").join("src");
    std::fs::read_dir(src)
        .ok()?
        .flatten()
        .map(|index| {
            index
                .path()
                .join(format!("steamworks-sys-{SYS_VERSION}"))
                .join("lib")
                .join("steam")
        })
        .find(|p| p.is_dir())
}

/// Where `cargo metadata` says steamworks-sys' manifest is.
fn from_metadata() -> Option<PathBuf> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?).join("Cargo.toml");
    // Filtered to the target: offline, cargo can only describe the packages
    // it has downloaded, and those are the target's (build-dependencies, which
    // name no platform, stay in). The build's own flags are dropped: a cross
    // build's (cargo-xwin's lld-link) break cargo's probe of the platform.
    let mut cmd = std::process::Command::new(cargo);
    cmd.args(["metadata", "--format-version", "1", "--offline"])
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS");
    if let Ok(target) = env::var("TARGET") {
        cmd.args(["--filter-platform", &target]);
    }
    let out = cmd.arg("--manifest-path").arg(manifest).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    meta["packages"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == "steamworks-sys" && p["version"] == SYS_VERSION)
        .and_then(|p| p["manifest_path"].as_str())
        .map(|m| {
            Path::new(m)
                .parent()
                .unwrap_or(Path::new("."))
                .join("lib")
                .join("steam")
        })
}
