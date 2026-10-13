//! The few Win32 calls the app makes itself, declared by hand rather than
//! through a bindings crate: reading and writing a registry string, making an
//! NTFS junction, riding out the short locks antivirus scanners take on a
//! freshly written file, reading a service's state and running a program
//! as administrator.
//!
//! Each replaces a helper process (`reg query`, `cmd /c mklink /J`) that cost
//! a process start, and a Defender scan of it, every time it ran.

use std::ffi::{OsString, c_void};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, Prefix};
use std::time::Duration;

type Handle = *mut c_void;
type HKey = isize;

/// `HKEY_CURRENT_USER`: `(HKEY)(ULONG_PTR)(LONG)0x80000001`, sign-extended.
pub const HKEY_CURRENT_USER: HKey = 0x8000_0001_u32 as i32 as isize;
/// `HKEY_LOCAL_MACHINE`, likewise.
pub const HKEY_LOCAL_MACHINE: HKey = 0x8000_0002_u32 as i32 as isize;

const RRF_RT_REG_SZ: u32 = 0x0000_0002;
const RRF_RT_ANY: u32 = 0x0000_FFFF;
const REG_SZ: u32 = 1;
const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
const ERROR_SUCCESS: i32 = 0;
const ERROR_MORE_DATA: i32 = 234;

const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00A4;
const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        hkey: HKey,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
    fn RegSetKeyValueW(
        hkey: HKey,
        subkey: *const u16,
        value: *const u16,
        kind: u32,
        data: *const c_void,
        size: u32,
    ) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> Handle;
    fn OpenServiceW(manager: Handle, name: *const u16, access: u32) -> Handle;
    fn QueryServiceStatus(service: Handle, status: *mut ServiceStatus) -> i32;
    fn QueryServiceConfigW(
        service: Handle,
        config: *mut c_void,
        size: u32,
        needed: *mut u32,
    ) -> i32;
    fn CloseServiceHandle(handle: Handle) -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHChangeNotify(event: i32, flags: u32, item1: *const c_void, item2: *const c_void);
    fn ShellExecuteExW(info: *mut ShellExecuteInfo) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn DeviceIoControl(
        handle: Handle,
        code: u32,
        input: *const c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
}

fn wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// A `REG_SZ` (or `REG_EXPAND_SZ`, expanded) value under `root\subkey`.
/// `Ok(None)` when the key or the value does not exist.
pub fn reg_string(root: HKey, subkey: &str, value: &str) -> io::Result<Option<String>> {
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buf: Vec<u16> = vec![0; 260];
    loop {
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: both names are NUL-terminated, `buf` holds `size` bytes.
        let status = unsafe {
            RegGetValueW(
                root,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        match status {
            ERROR_SUCCESS => {
                let len = (size as usize / 2).min(buf.len());
                let s = &buf[..len];
                let s = s.iter().position(|&c| c == 0).map_or(s, |end| &s[..end]);
                return Ok(Some(OsString::from_wide(s).to_string_lossy().into_owned()));
            }
            ERROR_MORE_DATA => buf.resize((size as usize).div_ceil(2) + 1, 0),
            // ERROR_FILE_NOT_FOUND: no such key or value.
            2 => return Ok(None),
            code => return Err(io::Error::from_raw_os_error(code)),
        }
    }
}

/// A number stored as `REG_QWORD`, `REG_DWORD` or a little-endian
/// `REG_BINARY` of up to eight bytes, as drivers write sizes.
/// `Ok(None)` when the key or the value does not exist.
pub fn reg_u64(root: HKey, subkey: &str, value: &str) -> io::Result<Option<u64>> {
    let (subkey, value) = (wide(subkey), wide(value));
    let mut buf = [0u8; 8];
    let mut size = buf.len() as u32;
    // SAFETY: both names are NUL-terminated, `buf` holds `size` bytes.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_ANY,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    match status {
        ERROR_SUCCESS => Ok(Some(u64::from_le_bytes(buf))),
        2 => Ok(None),
        code => Err(io::Error::from_raw_os_error(code)),
    }
}

/// Set the `REG_SZ` value `value` (the key's default when `None`) under
/// `root\subkey`, making the key if it is missing.
pub fn reg_set_string(root: HKey, subkey: &str, value: Option<&str>, data: &str) -> io::Result<()> {
    let subkey = wide(subkey);
    let value = value.map(wide);
    let data = wide(data);
    // SAFETY: the names and the data are NUL-terminated; `size` counts the
    // data's bytes, terminator included, as REG_SZ wants.
    let status = unsafe {
        RegSetKeyValueW(
            root,
            subkey.as_ptr(),
            value.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status))
    }
}

/// A drive a scan may touch: a local disk or a removable one. A network
/// drive (mapped share, possibly disconnected) or a missing letter is not:
/// asking a dead share can stall for a long time. `root` is like `"D:\\"`.
pub fn is_local_drive(root: &str) -> bool {
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    let root = wide(root);
    // SAFETY: `root` is NUL-terminated.
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    kind == DRIVE_FIXED || kind == DRIVE_REMOVABLE
}

/// Tell Explorer that file associations changed, so a new one shows at once.
pub fn associations_changed() {
    // SAFETY: SHCNF_IDLIST (0) with no items is the documented form for
    // SHCNE_ASSOCCHANGED.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, 0, std::ptr::null(), std::ptr::null()) };
}

/// Make `link` (which must not exist) an NTFS junction to the directory
/// `target`. Unlike a symbolic link, a junction needs no administrator
/// rights nor Developer Mode; it can only point at a local volume.
pub fn create_junction(target: &Path, link: &Path) -> io::Result<()> {
    let target = std::path::absolute(target)?;
    // The reparse data holds the NT path (`\??\C:\…`) and the path shown to
    // the user (`C:\…`).
    let mut print = Wide::default();
    let mut components = target.components();
    match components.next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                print.push_str(&format!("{}:", d as char));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "a junction can only point to a local drive",
                ));
            }
        },
        _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
    }
    for c in components {
        match c {
            Component::RootDir => {}
            Component::Normal(n) => {
                print.push_str("\\");
                print.push_os(n);
            }
            _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
        }
    }
    if print.0.len() == 2 {
        print.push_str("\\");
    }
    let print = print.0;
    let subst: Vec<u16> = "\\??\\"
        .encode_utf16()
        .chain(print.iter().copied())
        .collect();

    // REPARSE_DATA_BUFFER, MountPointReparseBuffer variant.
    let path_bytes = (subst.len() + 1 + print.len() + 1) * 2;
    let data_len = 8 + path_bytes;
    if data_len > 16 * 1024 - 8 {
        return Err(io::Error::from(io::ErrorKind::InvalidFilename));
    }
    let mut buf: Vec<u8> = Vec::with_capacity(8 + data_len);
    buf.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
    buf.extend_from_slice(&(data_len as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // SubstituteNameOffset
    buf.extend_from_slice(&((subst.len() * 2) as u16).to_le_bytes());
    buf.extend_from_slice(&(((subst.len() + 1) * 2) as u16).to_le_bytes()); // PrintNameOffset
    buf.extend_from_slice(&((print.len() * 2) as u16).to_le_bytes());
    for c in subst.iter().chain([&0]).chain(print.iter()).chain([&0]) {
        buf.extend_from_slice(&c.to_le_bytes());
    }

    std::fs::create_dir(link)?;
    let made = (|| {
        let name = wide(link);
        // SAFETY: `name` is NUL-terminated; the handle is closed below.
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_ALL,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut returned = 0u32;
        // SAFETY: `buf` is a complete REPARSE_DATA_BUFFER of `buf.len()` bytes.
        let ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_SET_REPARSE_POINT,
                buf.as_ptr().cast(),
                buf.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        let result = if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        };
        // SAFETY: `handle` is the valid handle opened above.
        unsafe { CloseHandle(handle) };
        result
    })();
    if made.is_err() {
        let _ = std::fs::remove_dir(link);
    }
    made
}

#[derive(Default)]
struct Wide(Vec<u16>);

impl Wide {
    fn push_str(&mut self, s: &str) {
        self.0.extend(s.encode_utf16());
    }
    fn push_os(&mut self, s: &std::ffi::OsStr) {
        self.0.extend(s.encode_wide());
    }
}

/// Whether `e` is the kind of failure a scanner or the indexer holding the
/// file for a moment causes: sharing violation, lock violation, access
/// denied.
pub fn is_transient_lock(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

/// Run `op` again, a few times over about a second and a half, while it fails with a
/// [transient lock](is_transient_lock). Windows Defender opens a file it
/// has just seen written, and a rename over it fails until it lets go.
pub fn retry_locked<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut wait = Duration::from_millis(25);
    for _ in 0..6 {
        match op() {
            Err(e) if is_transient_lock(&e) => std::thread::sleep(wait),
            other => return other,
        }
        wait *= 2;
    }
    op()
}

/// `SERVICE_STATUS`. Filled by the system: most fields are never read.
#[allow(dead_code)]
#[repr(C)]
#[derive(Default)]
struct ServiceStatus {
    kind: u32,
    current_state: u32,
    controls_accepted: u32,
    win32_exit_code: u32,
    specific_exit_code: u32,
    check_point: u32,
    wait_hint: u32,
}

/// What the service manager says of an installed service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    /// `SERVICE_RUNNING`, or on its way there (`SERVICE_START_PENDING`).
    pub running: bool,
    /// Its start type is `SERVICE_DISABLED`: nothing may start it.
    pub disabled: bool,
    /// The program it runs, without its quotes and arguments.
    pub binary: Option<std::path::PathBuf>,
}

/// Closes a service manager handle when dropped.
struct ScHandle(Handle);

impl Drop for ScHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenSCManagerW / OpenServiceW.
        unsafe { CloseServiceHandle(self.0) };
    }
}

/// The service `name` as the service manager knows it; `Ok(None)` when no
/// such service is installed. Needs no administrator rights.
pub fn service(name: &str) -> io::Result<Option<ServiceInfo>> {
    const SC_MANAGER_CONNECT: u32 = 0x0001;
    const SERVICE_QUERY_CONFIG: u32 = 0x0001;
    const SERVICE_QUERY_STATUS: u32 = 0x0004;
    const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
    const ERROR_INSUFFICIENT_BUFFER: i32 = 122;
    const SERVICE_START_PENDING: u32 = 2;
    const SERVICE_RUNNING: u32 = 4;
    const SERVICE_DISABLED: u32 = 4;

    // SAFETY: null machine and database name the local, active database.
    let manager = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err(io::Error::last_os_error());
    }
    let manager = ScHandle(manager);
    let name = wide(name);
    // SAFETY: `manager` is open, `name` NUL-terminated.
    let service = unsafe {
        OpenServiceW(
            manager.0,
            name.as_ptr(),
            SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS,
        )
    };
    if service.is_null() {
        let e = io::Error::last_os_error();
        return match e.raw_os_error() {
            Some(ERROR_SERVICE_DOES_NOT_EXIST) => Ok(None),
            _ => Err(e),
        };
    }
    let service = ScHandle(service);

    let mut status = ServiceStatus::default();
    // SAFETY: `service` is open with SERVICE_QUERY_STATUS.
    if unsafe { QueryServiceStatus(service.0, &mut status) } == 0 {
        return Err(io::Error::last_os_error());
    }

    // QUERY_SERVICE_CONFIGW and the strings after it: asked for its size
    // first. Held in u64s so its pointers are aligned.
    let mut needed = 0u32;
    // SAFETY: a null buffer of size 0 only asks for the size.
    unsafe { QueryServiceConfigW(service.0, std::ptr::null_mut(), 0, &mut needed) };
    let e = io::Error::last_os_error();
    if e.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER) {
        return Err(e);
    }
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `needed` bytes.
    if unsafe { QueryServiceConfigW(service.0, buf.as_mut_ptr().cast(), needed, &mut needed) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    /// The head of `QUERY_SERVICE_CONFIGW`.
    #[allow(dead_code)]
    #[repr(C)]
    struct Config {
        kind: u32,
        start_type: u32,
        error_control: u32,
        binary_path: *const u16,
    }
    // SAFETY: the call above filled `buf` with a QUERY_SERVICE_CONFIGW.
    let config = unsafe { &*buf.as_ptr().cast::<Config>() };
    // SAFETY: a NUL-terminated string inside `buf`, or null.
    let binary = (!config.binary_path.is_null())
        .then(|| unsafe {
            let len = (0..)
                .take_while(|&i| *config.binary_path.add(i) != 0)
                .count();
            OsString::from_wide(std::slice::from_raw_parts(config.binary_path, len))
        })
        .map(|cmd| service_program(&cmd.to_string_lossy()));

    Ok(Some(ServiceInfo {
        running: matches!(
            status.current_state,
            SERVICE_RUNNING | SERVICE_START_PENDING
        ),
        disabled: config.start_type == SERVICE_DISABLED,
        binary,
    }))
}

/// The program at the head of a service's command line:
/// `"C:\Program Files\X\x.exe" -a` and `C:\X\x.exe -a` give the path.
fn service_program(cmd: &str) -> std::path::PathBuf {
    let cmd = cmd.trim();
    let path = match cmd.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or(rest),
        // Unquoted: up to the end of `.exe`, since the path may hold spaces.
        None => cmd
            .to_ascii_lowercase()
            .find(".exe")
            .map_or(cmd, |i| &cmd[..i + 4]),
    };
    std::path::PathBuf::from(path)
}

/// `SHELLEXECUTEINFOW`. Read by the system: most fields are never read here.
#[allow(dead_code)]
#[repr(C)]
struct ShellExecuteInfo {
    size: u32,
    mask: u32,
    hwnd: Handle,
    verb: *const u16,
    file: *const u16,
    parameters: *const u16,
    directory: *const u16,
    show: i32,
    instance: Handle,
    id_list: *mut c_void,
    class: *const u16,
    class_key: Handle,
    hot_key: u32,
    icon_or_monitor: Handle,
    process: Handle,
}

/// Run `program` with `parameters` (one command-line string) in `directory`
/// as administrator, through the UAC prompt, and wait for it to finish.
/// Returns its exit code. Refusing the prompt is an error of kind
/// [`io::ErrorKind::Interrupted`]. Blocking.
pub fn run_elevated(program: &Path, parameters: &str, directory: &Path) -> io::Result<u32> {
    const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
    const SEE_MASK_NOASYNC: u32 = 0x0000_0100;
    const SW_SHOWNORMAL: i32 = 1;
    const INFINITE: u32 = u32::MAX;
    const ERROR_CANCELLED: i32 = 1223;

    let (verb, file, params, dir) = (
        wide("runas"),
        wide(program),
        wide(parameters),
        wide(directory),
    );
    let mut info = ShellExecuteInfo {
        size: std::mem::size_of::<ShellExecuteInfo>() as u32,
        mask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        hwnd: std::ptr::null_mut(),
        verb: verb.as_ptr(),
        file: file.as_ptr(),
        parameters: params.as_ptr(),
        directory: dir.as_ptr(),
        show: SW_SHOWNORMAL,
        instance: std::ptr::null_mut(),
        id_list: std::ptr::null_mut(),
        class: std::ptr::null(),
        class_key: std::ptr::null_mut(),
        hot_key: 0,
        icon_or_monitor: std::ptr::null_mut(),
        process: std::ptr::null_mut(),
    };
    // SAFETY: every string is NUL-terminated and outlives the call.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let e = io::Error::last_os_error();
        return Err(match e.raw_os_error() {
            Some(ERROR_CANCELLED) => io::Error::new(io::ErrorKind::Interrupted, e),
            _ => e,
        });
    }
    if info.process.is_null() {
        return Ok(0);
    }
    let mut code = 0u32;
    // SAFETY: `process` is the started program's handle, closed below.
    unsafe {
        WaitForSingleObject(info.process, INFINITE);
        GetExitCodeProcess(info.process, &mut code);
        CloseHandle(info.process);
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::service_program;
    use std::path::PathBuf;

    #[test]
    fn a_services_program_is_the_head_of_its_command_line() {
        assert_eq!(
            service_program(r#""C:\Program Files (x86)\Common Files\BattlEye\BEService.exe" -x"#),
            PathBuf::from(r"C:\Program Files (x86)\Common Files\BattlEye\BEService.exe")
        );
        assert_eq!(
            service_program(r"C:\Program Files\BE\BEService.EXE -x"),
            PathBuf::from(r"C:\Program Files\BE\BEService.EXE")
        );
    }
}
