//! The flat C API of Valve's library, resolved by name at run time.
//!
//! Names and signatures follow `steam_api_flat.h`, `steam_api.h` and
//! `steam_api_internal.h` of the SDK steamworks-sys 0.13.0 bundles
//! (ISteamUGC v021, ISteamUser v023). Every function is `extern "C"`
//! (`S_CALLTYPE` is `__cdecl`, the only convention on x86_64). A C++ `bool`
//! is one byte, as Rust's is.

use std::ffi::{c_char, c_void};
use std::sync::OnceLock;

/// `HSteamPipe`.
pub(crate) type Pipe = i32;
/// `SteamAPICall_t`.
pub(crate) type ApiCall = u64;
/// An `ISteam*` interface pointer.
pub(crate) type Iface = *mut c_void;

/// `ESteamAPIInitResult`.
pub(crate) const INIT_OK: i32 = 0;
pub(crate) const INIT_NO_STEAM_CLIENT: i32 = 2;
pub(crate) const INIT_VERSION_MISMATCH: i32 = 3;
/// `k_cchMaxSteamErrMsg`.
pub(crate) const ERR_MSG_LEN: usize = 1024;

/// `CallbackMsg_t` (the same layout under Valve's pack(4) and pack(8)).
#[repr(C)]
pub(crate) struct CallbackMsg {
    pub user: i32,
    pub callback: i32,
    pub param: *mut u8,
    pub size: i32,
}

impl Default for CallbackMsg {
    fn default() -> Self {
        Self {
            user: 0,
            callback: 0,
            param: std::ptr::null_mut(),
            size: 0,
        }
    }
}

/// The functions used, copied out of the library, which stays loaded for
/// the life of the process: Steam may keep threads running in it after a
/// shutdown, and unloading it under them would crash.
pub(crate) struct Api {
    _lib: libloading::Library,
    pub init_flat: unsafe extern "C" fn(*mut c_char) -> i32,
    pub shutdown: unsafe extern "C" fn(),
    pub is_steam_running: unsafe extern "C" fn() -> bool,
    pub get_pipe: unsafe extern "C" fn() -> Pipe,
    pub dispatch_init: unsafe extern "C" fn(),
    pub dispatch_run_frame: unsafe extern "C" fn(Pipe),
    pub dispatch_next: unsafe extern "C" fn(Pipe, *mut CallbackMsg) -> bool,
    pub dispatch_free_last: unsafe extern "C" fn(Pipe),
    pub dispatch_call_result:
        unsafe extern "C" fn(Pipe, ApiCall, *mut c_void, i32, i32, *mut bool) -> bool,
    pub user: unsafe extern "C" fn() -> Iface,
    pub logged_on: unsafe extern "C" fn(Iface) -> bool,
    pub ugc: unsafe extern "C" fn() -> Iface,
    pub subscribe: unsafe extern "C" fn(Iface, u64) -> ApiCall,
    pub unsubscribe: unsafe extern "C" fn(Iface, u64) -> ApiCall,
    pub num_subscribed: unsafe extern "C" fn(Iface, bool) -> u32,
    pub subscribed_items: unsafe extern "C" fn(Iface, *mut u64, u32, bool) -> u32,
    pub download: unsafe extern "C" fn(Iface, u64, bool) -> bool,
    pub item_state: unsafe extern "C" fn(Iface, u64) -> u32,
    pub download_info: unsafe extern "C" fn(Iface, u64, *mut u64, *mut u64) -> bool,
    pub install_info:
        unsafe extern "C" fn(Iface, u64, *mut u64, *mut c_char, u32, *mut u32) -> bool,
}

// SAFETY: the struct holds only function pointers and the library handle;
// calling them is confined to the one session thread (see session.rs).
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

static API: OnceLock<Result<Api, String>> = OnceLock::new();

/// The library, extracted and loaded on first call; the same answer after.
pub(crate) fn get() -> Result<&'static Api, String> {
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, String> {
    let Some((name, bytes)) = crate::redist::EMBEDDED else {
        return Err("This build carries no Steamworks library for this system".into());
    };
    let path = crate::redist::extract(&dz_common::paths::steamworks_dir(), name, bytes)
        .map_err(|e| format!("Valve's library could not be written to disk ({e})"))?;
    // SAFETY: Valve's redistributable, written just above from the bytes
    // embedded at build time; its initialisers only set up its own state.
    let lib = unsafe { libloading::Library::new(&path) }
        .map_err(|e| format!("{} did not load ({e})", path.display()))?;

    macro_rules! sym {
        ($name:literal) => {
            // SAFETY: the type is the one steam_api_flat.h / steam_api.h
            // declare for this name in the SDK the library comes from.
            *unsafe { lib.get($name) }
                .map_err(|e| format!("{} is missing from Valve's library ({e})", $name))?
        };
    }
    Ok(Api {
        init_flat: sym!("SteamAPI_InitFlat"),
        shutdown: sym!("SteamAPI_Shutdown"),
        is_steam_running: sym!("SteamAPI_IsSteamRunning"),
        get_pipe: sym!("SteamAPI_GetHSteamPipe"),
        dispatch_init: sym!("SteamAPI_ManualDispatch_Init"),
        dispatch_run_frame: sym!("SteamAPI_ManualDispatch_RunFrame"),
        dispatch_next: sym!("SteamAPI_ManualDispatch_GetNextCallback"),
        dispatch_free_last: sym!("SteamAPI_ManualDispatch_FreeLastCallback"),
        dispatch_call_result: sym!("SteamAPI_ManualDispatch_GetAPICallResult"),
        user: sym!("SteamAPI_SteamUser_v023"),
        logged_on: sym!("SteamAPI_ISteamUser_BLoggedOn"),
        ugc: sym!("SteamAPI_SteamUGC_v021"),
        subscribe: sym!("SteamAPI_ISteamUGC_SubscribeItem"),
        unsubscribe: sym!("SteamAPI_ISteamUGC_UnsubscribeItem"),
        num_subscribed: sym!("SteamAPI_ISteamUGC_GetNumSubscribedItems"),
        subscribed_items: sym!("SteamAPI_ISteamUGC_GetSubscribedItems"),
        download: sym!("SteamAPI_ISteamUGC_DownloadItem"),
        item_state: sym!("SteamAPI_ISteamUGC_GetItemState"),
        download_info: sym!("SteamAPI_ISteamUGC_GetItemDownloadInfo"),
        install_info: sym!("SteamAPI_ISteamUGC_GetItemInstallInfo"),
        _lib: lib,
    })
}
