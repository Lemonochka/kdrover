use once_cell::sync::OnceCell;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

use drover_core::{
    discord, load_options, options, proxy::ProxyValue, socket_manager::SocketManager, DroverOptions,
};

pub struct DroverState {
    pub process_dir: PathBuf,
    pub socket_manager: SocketManager,
    pub options: DroverOptions,
    pub proxy: ProxyValue,
    pub command_line_wide: &'static [u16],
}

static STATE: OnceCell<DroverState> = OnceCell::new();

pub fn state() -> &'static DroverState {
    STATE.get().expect("drover state is not initialized")
}

pub fn init_state() {
    STATE.get_or_init(|| {
        let process_dir = current_process_dir();
        let options = load_options(process_dir.join(options::OPTIONS_FILENAME));
        let proxy = ProxyValue::parse_from_string(&options.proxy);
        let command_line = build_command_line_cache(&proxy);
        let command_line_wide = leak_wide_string(&command_line);

        DroverState {
            process_dir,
            socket_manager: SocketManager::new(),
            options,
            proxy: proxy.clone(),
            command_line_wide,
        }
    });
}

fn build_command_line_cache(proxy: &ProxyValue) -> String {
    let mut command_line = read_command_line_w();

    if proxy.is_specified {
        let exe_name = current_exe_name();
        if discord::is_discord_executable(&exe_name) {
            command_line.push_str(" --proxy-server=");
            command_line.push_str(&proxy.format_to_chrome_proxy());
        }
    }

    command_line
}

fn leak_wide_string(value: &str) -> &'static [u16] {
    let wide: Vec<u16> = OsStr::new(value).encode_wide().chain([0]).collect();
    wide.leak()
}

fn current_process_dir() -> PathBuf {
    let mut buffer = vec![0u16; 32_768];
    let len = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleFileNameW(
            windows::Win32::Foundation::HMODULE::default(),
            &mut buffer,
        )
    };

    if len == 0 {
        return PathBuf::from(".");
    }

    let path = OsString::from_wide(&buffer[..len as usize]);
    PathBuf::from(path)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn current_exe_name() -> String {
    let mut buffer = vec![0u16; 32_768];
    let len = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleFileNameW(
            windows::Win32::Foundation::HMODULE::default(),
            &mut buffer,
        )
    };

    if len == 0 {
        return String::new();
    }

    std::path::Path::new(&OsString::from_wide(&buffer[..len as usize]))
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_default()
}

fn read_command_line_w() -> String {
    type FnGetCommandLineW = unsafe extern "system" fn() -> *const u16;

    let wide: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    let kernel32 = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::PCWSTR(wide.as_ptr()))
    }
    .unwrap_or_default();
    let symbol = b"GetCommandLineW\0";
    let address = unsafe {
        windows::Win32::System::LibraryLoader::GetProcAddress(
            kernel32,
            windows::core::PCSTR(symbol.as_ptr()),
        )
    };

    let Some(address) = address else {
        return String::new();
    };

    let get_command_line_w: FnGetCommandLineW = unsafe { std::mem::transmute(address) };
    let ptr = unsafe { get_command_line_w() };
    if ptr.is_null() {
        return String::new();
    }

    unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

pub fn copy_files_to_all_discord_dirs() {
    let state = state();
    let source_options = state.process_dir.join(options::OPTIONS_FILENAME);
    let source_dll = state.process_dir.join(options::DLL_FILENAME);

    if !source_options.exists() || !source_dll.exists() {
        return;
    }

    let base_dir = state
        .process_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| state.process_dir.clone());

    for dir in discord::find_discord_app_dirs(&base_dir) {
        let dst_dll = dir.join(options::DLL_FILENAME);

        // Trigger on a missing version.dll alone: a freshly downloaded app-X.Y.Z folder
        // has Discord.exe but none of our files, and that is exactly the dir we must
        // populate before Discord restarts into it.
        if discord::dir_has_discord_executable(&dir) && !dst_dll.exists() {
            let _ = options::copy_drover_files(&state.process_dir, &dir);
        }
    }
}
