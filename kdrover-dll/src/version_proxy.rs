use std::ffi::c_void;
use std::sync::OnceLock;
use windows::core::PCSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GetProcAddress, LoadLibraryExW, LoadLibraryW,
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, LOAD_WITH_ALTERED_SEARCH_PATH,
};

static ORIGINAL_VERSION: OnceLock<isize> = OnceLock::new();

pub fn init_version_proxy() -> windows::core::Result<()> {
    if ORIGINAL_VERSION.get().is_some() {
        return Ok(());
    }

    let module = load_system_version_dll()?;
    ORIGINAL_VERSION
        .set(module.0 as isize)
        .map_err(|_| windows::core::Error::from_win32())
}

fn load_system_version_dll() -> windows::core::Result<HMODULE> {
    let system_dir = system_directory()?;
    let path = format!("{system_dir}\\version.dll");
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();

    // Same as original Drover: full path into System32/SysWOW64.
    if let Ok(module) = unsafe { LoadLibraryW(windows::core::PCWSTR(wide.as_ptr())) } {
        if module_is_usable(module) {
            return Ok(module);
        }
    }

    if let Ok(module) = unsafe {
        LoadLibraryExW(
            windows::core::PCWSTR(wide.as_ptr()),
            None,
            LOAD_WITH_ALTERED_SEARCH_PATH,
        )
    } {
        if module_is_usable(module) {
            return Ok(module);
        }
    }

    Err(windows::core::Error::from_win32())
}

fn module_is_usable(module: HMODULE) -> bool {
    if module.is_invalid() {
        return false;
    }

    // If Windows returned our own hijack DLL, reject it.
    if let Some(self_handle) = current_module_handle() {
        if module.0 == self_handle.0 {
            return false;
        }
    }

    true
}

fn current_module_handle() -> Option<HMODULE> {
    let mut handle = HMODULE::default();
    let address = GetFileVersionInfoW as *const c_void;
    let ok = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            windows::core::PCWSTR(address as *const u16),
            &mut handle,
        )
    };
    if ok.is_ok() && !handle.is_invalid() {
        Some(handle)
    } else {
        None
    }
}

fn system_directory() -> windows::core::Result<String> {
    let mut buffer = vec![0u16; 260];
    let len =
        unsafe { windows::Win32::System::SystemInformation::GetSystemDirectoryW(Some(&mut buffer)) };
    if len == 0 {
        return Err(windows::core::Error::from_win32());
    }
    Ok(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn get_proc(name: &str) -> Option<*const c_void> {
    // Warm handle from the worker thread in the common case; lazily load on demand if a
    // version export arrives before the worker resolved it.
    let handle = match ORIGINAL_VERSION.get().copied() {
        Some(handle) => handle,
        None => {
            let module = load_system_version_dll().ok()?;
            let _ = ORIGINAL_VERSION.set(module.0 as isize);
            ORIGINAL_VERSION.get().copied()?
        }
    };
    let module = HMODULE(handle as _);
    let name = format!("{name}\0");
    unsafe {
        GetProcAddress(module, PCSTR(name.as_ptr())).map(|func| func as *const c_void)
    }
}

macro_rules! forward_export {
    ($name:ident, $ty:ty, ($($arg:ident : $t:ty),*) -> $ret:ty) => {
        #[no_mangle]
        pub unsafe extern "system" fn $name($($arg: $t),*) -> $ret {
            let Some(proc) = get_proc(stringify!($name)) else {
                return Default::default();
            };
            let func: $ty = std::mem::transmute(proc);
            func($($arg),*)
        }
    };
}

type GetFileVersionInfoAFn = unsafe extern "system" fn(*const u8, u32, u32, *mut c_void) -> i32;
type GetFileVersionInfoByHandleFn =
    unsafe extern "system" fn(isize, *const u8, u32, u32, *mut c_void) -> i32;
type GetFileVersionInfoExAFn =
    unsafe extern "system" fn(u32, *const u8, u32, u32, *mut c_void) -> i32;
type GetFileVersionInfoExWFn =
    unsafe extern "system" fn(u32, *const u16, u32, u32, *mut c_void) -> i32;
type GetFileVersionInfoSizeAFn = unsafe extern "system" fn(*const u8, *mut u32) -> u32;
type GetFileVersionInfoSizeExAFn = unsafe extern "system" fn(u32, *const u8, *mut u32) -> u32;
type GetFileVersionInfoSizeExWFn = unsafe extern "system" fn(u32, *const u16, *mut u32) -> u32;
type GetFileVersionInfoSizeWFn = unsafe extern "system" fn(*const u16, *mut u32) -> u32;
type GetFileVersionInfoWFn = unsafe extern "system" fn(*const u16, u32, u32, *mut c_void) -> i32;
type VerFindFileAFn = unsafe extern "system" fn(
    u32,
    *const u8,
    *const u8,
    *const u8,
    *mut u8,
    *mut u32,
    *mut u8,
    *mut u32,
) -> u32;
type VerFindFileWFn = unsafe extern "system" fn(
    u32,
    *const u16,
    *const u16,
    *const u16,
    *mut u16,
    *mut u32,
    *mut u16,
    *mut u32,
) -> u32;
type VerInstallFileAFn = unsafe extern "system" fn(
    u32,
    *const u8,
    *const u8,
    *const u8,
    *const u8,
    *const u8,
    *mut u8,
    u32,
) -> u32;
type VerInstallFileWFn = unsafe extern "system" fn(
    u32,
    *const u16,
    *const u16,
    *const u16,
    *const u16,
    *const u16,
    *mut u16,
    u32,
) -> u32;
type VerLanguageNameAFn = unsafe extern "system" fn(u32, *mut u8, u32) -> u32;
type VerLanguageNameWFn = unsafe extern "system" fn(u32, *mut u16, u32) -> u32;
type VerQueryValueAFn =
    unsafe extern "system" fn(*const c_void, *const u8, *mut *mut c_void, *mut u32) -> i32;
type VerQueryValueWFn =
    unsafe extern "system" fn(*const c_void, *const u16, *mut *mut c_void, *mut u32) -> i32;

forward_export!(GetFileVersionInfoA, GetFileVersionInfoAFn, (lptstr_filename: *const u8, dw_handle: u32, dw_len: u32, lp_data: *mut c_void) -> i32);
forward_export!(GetFileVersionInfoByHandle, GetFileVersionInfoByHandleFn, (u_handle: isize, lptstr_filename: *const u8, dw_handle: u32, dw_len: u32, lp_data: *mut c_void) -> i32);
forward_export!(GetFileVersionInfoExA, GetFileVersionInfoExAFn, (dw_flags: u32, lptstr_filename: *const u8, dw_handle: u32, dw_len: u32, lp_data: *mut c_void) -> i32);
forward_export!(GetFileVersionInfoExW, GetFileVersionInfoExWFn, (dw_flags: u32, lptstr_filename: *const u16, dw_handle: u32, dw_len: u32, lp_data: *mut c_void) -> i32);
forward_export!(GetFileVersionInfoSizeA, GetFileVersionInfoSizeAFn, (lptstr_filename: *const u8, lpdw_handle: *mut u32) -> u32);
forward_export!(GetFileVersionInfoSizeExA, GetFileVersionInfoSizeExAFn, (dw_flags: u32, lptstr_filename: *const u8, lpdw_handle: *mut u32) -> u32);
forward_export!(GetFileVersionInfoSizeExW, GetFileVersionInfoSizeExWFn, (dw_flags: u32, lptstr_filename: *const u16, lpdw_handle: *mut u32) -> u32);
forward_export!(GetFileVersionInfoSizeW, GetFileVersionInfoSizeWFn, (lptstr_filename: *const u16, lpdw_handle: *mut u32) -> u32);
forward_export!(GetFileVersionInfoW, GetFileVersionInfoWFn, (lptstr_filename: *const u16, dw_handle: u32, dw_len: u32, lp_data: *mut c_void) -> i32);
forward_export!(VerFindFileA, VerFindFileAFn, (u_flags: u32, sz_filename: *const u8, sz_win_dir: *const u8, sz_app_dir: *const u8, sz_cur_dir: *mut u8, pu_cur_dir_len: *mut u32, sz_dest_dir: *mut u8, pu_dest_dir_len: *mut u32) -> u32);
forward_export!(VerFindFileW, VerFindFileWFn, (u_flags: u32, sz_filename: *const u16, sz_win_dir: *const u16, sz_app_dir: *const u16, sz_cur_dir: *mut u16, pu_cur_dir_len: *mut u32, sz_dest_dir: *mut u16, pu_dest_dir_len: *mut u32) -> u32);
forward_export!(VerInstallFileA, VerInstallFileAFn, (u_flags: u32, sz_src_filename: *const u8, sz_dest_filename: *const u8, sz_src_dir: *const u8, sz_dest_dir: *const u8, sz_cur_dir: *const u8, sz_tmp_file: *mut u8, pu_tmp_file_len: u32) -> u32);
forward_export!(VerInstallFileW, VerInstallFileWFn, (u_flags: u32, sz_src_filename: *const u16, sz_dest_filename: *const u16, sz_src_dir: *const u16, sz_dest_dir: *const u16, sz_cur_dir: *const u16, sz_tmp_file: *mut u16, pu_tmp_file_len: u32) -> u32);
forward_export!(VerLanguageNameA, VerLanguageNameAFn, (w_lang: u32, sz_lang: *mut u8, cch_lang: u32) -> u32);
forward_export!(VerLanguageNameW, VerLanguageNameWFn, (w_lang: u32, sz_lang: *mut u16, cch_lang: u32) -> u32);
forward_export!(VerQueryValueA, VerQueryValueAFn, (p_block: *const c_void, lp_sub_block: *const u8, lplp_buffer: *mut *mut c_void, pu_len: *mut u32) -> i32);
forward_export!(VerQueryValueW, VerQueryValueWFn, (p_block: *const c_void, lp_sub_block: *const u16, lplp_buffer: *mut *mut c_void, pu_len: *mut u32) -> i32);
