use once_cell::sync::OnceCell;
use retour::GenericDetour;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::Networking::WinSock::WSABUF;
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{PROCESS_INFORMATION, STARTUPINFOW};

use drover_core::{http_auth, is_discord_executable, socks5, udp};

use crate::state::{copy_files_to_all_discord_dirs, state};

type FnGetEnvironmentVariableW = unsafe extern "system" fn(*const u16, *mut u16, u32) -> u32;
type FnCreateProcessW = unsafe extern "system" fn(
    *const u16,
    *mut u16,
    *const SECURITY_ATTRIBUTES,
    *const SECURITY_ATTRIBUTES,
    BOOL,
    u32,
    *const c_void,
    *const u16,
    *const STARTUPINFOW,
    *mut PROCESS_INFORMATION,
) -> BOOL;
type FnGetCommandLineW = unsafe extern "system" fn() -> *const u16;
type FnSocket = unsafe extern "system" fn(i32, i32, i32) -> usize;
type FnWSASocket = unsafe extern "system" fn(i32, i32, i32, *mut c_void, u32, u32) -> usize;
type FnWSASend = unsafe extern "system" fn(
    usize,
    *const WSABUF,
    u32,
    *mut u32,
    u32,
    *mut OVERLAPPED,
    Option<unsafe extern "system" fn(u32, u32, *mut OVERLAPPED, u32)>,
) -> i32;
type FnWSASendTo = unsafe extern "system" fn(
    usize,
    *const WSABUF,
    u32,
    *mut u32,
    u32,
    *const c_void,
    i32,
    *mut OVERLAPPED,
    Option<unsafe extern "system" fn(u32, u32, *mut OVERLAPPED, u32)>,
) -> i32;
type FnSend = unsafe extern "system" fn(usize, *const u8, i32, i32) -> i32;
type FnRecv = unsafe extern "system" fn(usize, *mut u8, i32, i32) -> i32;
type FnSendTo = unsafe extern "system" fn(usize, *const u8, i32, i32, *const c_void, i32) -> i32;

struct Hooks {
    get_environment_variable_w: GenericDetour<FnGetEnvironmentVariableW>,
    create_process_w: GenericDetour<FnCreateProcessW>,
    get_command_line_w: GenericDetour<FnGetCommandLineW>,
    socket: GenericDetour<FnSocket>,
    wsa_socket: GenericDetour<FnWSASocket>,
    wsa_send: GenericDetour<FnWSASend>,
    wsa_send_to: GenericDetour<FnWSASendTo>,
    send: GenericDetour<FnSend>,
    recv: GenericDetour<FnRecv>,
    send_to: GenericDetour<FnSendTo>,
    raw_send_to: FnSendTo,
}

static HOOKS: OnceCell<Hooks> = OnceCell::new();

// SAFETY: `Hooks` is published into `HOOKS` only after every detour has been
// enabled, and is never mutated afterwards. Enabling/disabling happens solely on
// the worker thread before publication. Calling a `GenericDetour` trampoline is
// a thread-safe read (it just jumps to relocated code), so concurrent `&Hooks`
// access from any thread is sound. We must NOT guard reads with a lock: Winsock's
// `socket()` internally calls the hooked `WSASocketW`, so a single `socket()` call
// re-enters our detours on the same thread — a lock here would self-deadlock.
unsafe impl Sync for Hooks {}
unsafe impl Send for Hooks {}

pub fn install_hooks() -> Result<(), Box<dyn std::error::Error>> {
    if HOOKS.get().is_some() {
        return Ok(());
    }

    let kernel32 = module_handle("kernel32.dll")?;
    let ws2_32 = module_handle("ws2_32.dll")?;
    let raw_send_to: FnSendTo = resolve_symbol(ws2_32, "sendto")?;

    let hooks = Hooks {
        get_environment_variable_w: hook_from_module(
            kernel32,
            "GetEnvironmentVariableW",
            detour_get_environment_variable_w as FnGetEnvironmentVariableW,
        )?,
        create_process_w: hook_from_module(
            kernel32,
            "CreateProcessW",
            detour_create_process_w as FnCreateProcessW,
        )?,
        get_command_line_w: hook_from_module(
            kernel32,
            "GetCommandLineW",
            detour_get_command_line_w as FnGetCommandLineW,
        )?,
        socket: hook_from_module(ws2_32, "socket", detour_socket as FnSocket)?,
        wsa_socket: hook_from_module(ws2_32, "WSASocketW", detour_wsa_socket as FnWSASocket)
            .or_else(|_| hook_from_module(ws2_32, "WSASocketA", detour_wsa_socket as FnWSASocket))?,
        wsa_send: hook_from_module(ws2_32, "WSASend", detour_wsa_send as FnWSASend)?,
        wsa_send_to: hook_from_module(ws2_32, "WSASendTo", detour_wsa_send_to as FnWSASendTo)?,
        send: hook_from_module(ws2_32, "send", detour_send as FnSend)?,
        recv: hook_from_module(ws2_32, "recv", detour_recv as FnRecv)?,
        send_to: hook_from_module(ws2_32, "sendto", detour_send_to as FnSendTo)?,
        raw_send_to,
    };

    // Publish the (not-yet-enabled) hooks BEFORE enabling any of them. Once a hook
    // is enabled its detour can fire on another thread and read `HOOKS` lock-free;
    // if we enabled first and published after, that read could observe an empty
    // cell and panic. While publishing, nothing is hooked yet, so no detour runs.
    if HOOKS.set(hooks).is_err() {
        return Err("hooks already installed".into());
    }
    let hooks = HOOKS.get().expect("hooks just published");

    unsafe {
        hooks.get_environment_variable_w.enable()?;
        hooks.create_process_w.enable()?;
        hooks.get_command_line_w.enable()?;
        hooks.socket.enable()?;
        hooks.wsa_socket.enable()?;
        hooks.wsa_send.enable()?;
        hooks.wsa_send_to.enable()?;
        hooks.send.enable()?;
        hooks.recv.enable()?;
        hooks.send_to.enable()?;
    }

    Ok(())
}

fn module_handle(name: &str) -> Result<HMODULE, Box<dyn std::error::Error>> {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    let ptr = windows::core::PCWSTR(wide.as_ptr());

    if let Ok(handle) = unsafe { GetModuleHandleW(ptr) } {
        if !handle.is_invalid() {
            return Ok(handle);
        }
    }

    unsafe { LoadLibraryW(ptr) }
        .map_err(|error| -> Box<dyn std::error::Error> { error.into() })
}

fn resolve_symbol<T>(module: HMODULE, name: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: Copy,
{
    let symbol = format!("{name}\0");
    let address = unsafe { GetProcAddress(module, windows::core::PCSTR(symbol.as_ptr())) }
        .ok_or_else(|| format!("failed to resolve {name}"))?;
    Ok(unsafe { std::mem::transmute_copy(&address) })
}

fn hook_from_module<T>(
    module: HMODULE,
    name: &str,
    detour: T,
) -> Result<GenericDetour<T>, Box<dyn std::error::Error>>
where
    T: Copy + retour::Function,
{
    let target: T = resolve_symbol(module, name)?;
    let hook = unsafe { GenericDetour::new(target, detour)? };
    Ok(hook)
}

fn with_hooks<F, R>(f: F) -> R
where
    F: FnOnce(&Hooks) -> R,
{
    // Lock-free read: see the SAFETY note on `HOOKS`. A lock here would deadlock
    // because the original functions we call below can re-enter our detours on
    // the same thread (e.g. `socket()` -> `WSASocketW`).
    f(HOOKS.get().expect("hooks are not installed"))
}

fn run_udp_voice_bypass(sock: usize, buffer_len: usize, to: *const c_void, to_len: i32, sendto: FnSendTo) {
    let drover = state();
    udp::maybe_send_udp_bypass_packets(
        drover.options.udp_bypass,
        &drover.process_dir,
        sendto,
        sock,
        to,
        to_len,
        buffer_len,
    );
}

unsafe extern "system" fn detour_get_environment_variable_w(
    lp_name: *const u16,
    lp_buffer: *mut u16,
    n_size: u32,
) -> u32 {
    let drover = state();
    if drover.proxy.is_specified && !lp_name.is_null() {
        let name = wide_ptr_to_string(lp_name);
        if name.eq_ignore_ascii_case("http_proxy") || name.eq_ignore_ascii_case("https_proxy") {
            let value = drover.proxy.format_to_http_env();
            let required = value.encode_utf16().count() as u32 + 1;
            if lp_buffer.is_null() || n_size < required {
                return required;
            }
            write_wide_string(&value, lp_buffer, n_size);
            return value.len() as u32;
        }
    }

    with_hooks(|hooks| unsafe {
        hooks
            .get_environment_variable_w
            .call(lp_name, lp_buffer, n_size)
    })
}

unsafe extern "system" fn detour_create_process_w(
    lp_application_name: *const u16,
    lp_command_line: *mut u16,
    lp_process_attributes: *const SECURITY_ATTRIBUTES,
    lp_thread_attributes: *const SECURITY_ATTRIBUTES,
    b_inherit_handles: BOOL,
    dw_creation_flags: u32,
    lp_environment: *const c_void,
    lp_current_directory: *const u16,
    lp_startup_info: *const STARTUPINFOW,
    lp_process_information: *mut PROCESS_INFORMATION,
) -> BOOL {
    if !lp_application_name.is_null() {
        let app_name = wide_ptr_to_os_string(lp_application_name).unwrap_or_default();
        let file_name = app_name
            .to_string_lossy()
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_string();

        if is_discord_executable(&file_name) || file_name.eq_ignore_ascii_case("reg.exe") {
            copy_files_to_all_discord_dirs();
        }
    }

    with_hooks(|hooks| unsafe {
        hooks.create_process_w.call(
            lp_application_name,
            lp_command_line,
            lp_process_attributes,
            lp_thread_attributes,
            b_inherit_handles,
            dw_creation_flags,
            lp_environment,
            lp_current_directory,
            lp_startup_info,
            lp_process_information,
        )
    })
}

unsafe extern "system" fn detour_get_command_line_w() -> *const u16 {
    state().command_line_wide.as_ptr()
}

unsafe extern "system" fn detour_socket(af: i32, sock_type: i32, protocol: i32) -> usize {
    with_hooks(|hooks| {
        let sock = unsafe { hooks.socket.call(af, sock_type, protocol) };
        state().socket_manager.add(sock, sock_type, protocol);
        sock
    })
}

unsafe extern "system" fn detour_wsa_socket(
    af: i32,
    sock_type: i32,
    protocol: i32,
    lp_protocol_info: *mut c_void,
    group: u32,
    dw_flags: u32,
) -> usize {
    with_hooks(|hooks| {
        let sock = unsafe {
            hooks
                .wsa_socket
                .call(af, sock_type, protocol, lp_protocol_info, group, dw_flags)
        };
        state().socket_manager.add(sock, sock_type, protocol);
        sock
    })
}

unsafe extern "system" fn detour_wsa_send(
    sock: usize,
    lp_buffers: *const WSABUF,
    dw_buffer_count: u32,
    lp_number_of_bytes_sent: *mut u32,
    dw_flags: u32,
    lp_overlapped: *mut OVERLAPPED,
    lp_completion_routine: Option<unsafe extern "system" fn(u32, u32, *mut OVERLAPPED, u32)>,
) -> i32 {
    if !lp_buffers.is_null() && dw_buffer_count == 1 {
        let buffer = unsafe { &*lp_buffers };
        if !buffer.buf.is_null() && buffer.len > 0 {
            let drover = state();
            if let Some(item) = drover.socket_manager.is_first_send(sock) {
                let mut packet = unsafe {
                    std::slice::from_raw_parts(buffer.buf.0 as *const u8, buffer.len as usize).to_vec()
                };
                if http_auth::add_http_proxy_authorization_header(&item, &drover.proxy, &mut packet) {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            packet.as_ptr(),
                            buffer.buf.0,
                            packet.len(),
                        );
                    }
                }
            }
        }
    }

    with_hooks(|hooks| unsafe {
        hooks.wsa_send.call(
            sock,
            lp_buffers,
            dw_buffer_count,
            lp_number_of_bytes_sent,
            dw_flags,
            lp_overlapped,
            lp_completion_routine,
        )
    })
}

unsafe extern "system" fn detour_wsa_send_to(
    sock: usize,
    lp_buffers: *const WSABUF,
    dw_buffer_count: u32,
    lp_number_of_bytes_sent: *mut u32,
    dw_flags: u32,
    lp_to: *const c_void,
    i_to_len: i32,
    lp_overlapped: *mut OVERLAPPED,
    lp_completion_routine: Option<unsafe extern "system" fn(u32, u32, *mut OVERLAPPED, u32)>,
) -> i32 {
    if !lp_buffers.is_null() && dw_buffer_count > 0 {
        let buffer = unsafe { &*lp_buffers };
        if let Some(item) = state().socket_manager.is_first_send(sock) {
            if item.is_udp && buffer.len == 74 {
                with_hooks(|hooks| {
                    run_udp_voice_bypass(
                        sock,
                        buffer.len as usize,
                        lp_to,
                        i_to_len,
                        hooks.raw_send_to,
                    );
                });
            }
        }
    }

    with_hooks(|hooks| unsafe {
        hooks.wsa_send_to.call(
            sock,
            lp_buffers,
            dw_buffer_count,
            lp_number_of_bytes_sent,
            dw_flags,
            lp_to,
            i_to_len,
            lp_overlapped,
            lp_completion_routine,
        )
    })
}

unsafe extern "system" fn detour_send_to(
    sock: usize,
    buf: *const u8,
    len: i32,
    flags: i32,
    to: *const c_void,
    to_len: i32,
) -> i32 {
    if len > 0 && !buf.is_null() {
        if let Some(item) = state().socket_manager.is_first_send(sock) {
            if item.is_udp && len as usize == 74 {
                with_hooks(|hooks| {
                    run_udp_voice_bypass(sock, len as usize, to, to_len, hooks.raw_send_to);
                });
            }
        }
    }

    with_hooks(|hooks| unsafe { hooks.send_to.call(sock, buf, len, flags, to, to_len) })
}

unsafe extern "system" fn detour_send(sock: usize, buf: *const u8, len: i32, flags: i32) -> i32 {
    let drover = state();
    if let Some(item) = drover.socket_manager.is_first_send(sock) {
        if len > 0 && !buf.is_null() {
            let data = unsafe { std::slice::from_raw_parts(buf, len as usize) };
            let converted = with_hooks(|hooks| unsafe {
                socks5::convert_http_connect_to_socks5(
                    &item,
                    &drover.proxy,
                    data,
                    flags,
                    |s, b, l, f| hooks.send.call(s, b, l, f),
                    |s, b, l, f| hooks.recv.call(s, b, l, f),
                )
            });
            if converted {
                drover.socket_manager.set_fake_http_proxy_flag(sock);
                return len;
            }
        }
    }

    with_hooks(|hooks| unsafe { hooks.send.call(sock, buf, len, flags) })
}

unsafe extern "system" fn detour_recv(sock: usize, buf: *mut u8, len: i32, flags: i32) -> i32 {
    let received = with_hooks(|hooks| unsafe { hooks.recv.call(sock, buf, len, flags) });
    if received > 0 && state().socket_manager.reset_fake_http_proxy_flag(sock) {
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, received as usize) };
        if let Some(new_len) = socks5::try_convert_socks5_reply_to_http(slice, received as usize) {
            return new_len as i32;
        }
    }
    received
}

fn wide_ptr_to_string(value: *const u16) -> String {
    wide_ptr_to_os_string(value)
        .and_then(|value| value.into_string().ok())
        .unwrap_or_default()
}

fn wide_ptr_to_os_string(value: *const u16) -> Option<OsString> {
    if value.is_null() {
        return None;
    }

    unsafe {
        let mut len = 0;
        while *value.add(len) != 0 {
            len += 1;
        }
        Some(OsString::from_wide(std::slice::from_raw_parts(value, len)))
    }
}

fn write_wide_string(value: &str, buffer: *mut u16, size: u32) {
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    let copy_len = wide.len().min(size as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), buffer, copy_len);
    }
}
