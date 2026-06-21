#![allow(non_snake_case, non_upper_case_globals)]

mod hooks;
mod state;
mod version_proxy;

use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, HANDLE, HMODULE};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::System::Threading::{CreateThread, THREAD_CREATION_FLAGS};

use crate::hooks::install_hooks;
use crate::state::{copy_files_to_all_discord_dirs, init_state};
use crate::version_proxy::init_version_proxy;

#[no_mangle]
pub unsafe extern "system" fn DllMain(module: HANDLE, reason: u32, _reserved: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            let _ = DisableThreadLibraryCalls(HMODULE(module.0));

            // Do NOTHING that can touch the loader here. LoadLibrary under the loader
            // lock that DllMain holds can deadlock: if the target (or one of its
            // dependencies) still needs its own DLL_PROCESS_ATTACH, the parallel loader
            // parks this thread in LdrpDrainWorkQueue waiting on a work queue this very
            // thread owns. That includes loading the real version.dll for the proxy.
            //
            // Defer everything — proxy resolution, option file I/O, ws2_32 load, detour
            // installation — to a worker thread, which only starts running once DllMain
            // returns and the loader lock is released. version exports that arrive before
            // the worker has resolved the real DLL fall back to a lazy load (see
            // version_proxy::get_proc).
            let _ = CreateThread(
                None,
                0,
                Some(init_thread),
                None,
                THREAD_CREATION_FLAGS(0),
                None,
            );
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }

    BOOL::from(true)
}

unsafe extern "system" fn init_thread(_param: *mut c_void) -> u32 {
    // Runs after the loader lock is released. Install hooks AS EARLY AS POSSIBLE: the
    // socket/sendto detours must be live before Discord creates its voice socket and
    // sends the first packet, otherwise the UDP bypass is missed and DPI throttles the
    // flow (ping climbs to ~5k after a few seconds). Anything ahead of install_hooks
    // here is pure latency on that race.
    init_state();
    let _ = install_hooks();

    // Resolve the real version.dll last — it is off the critical path and forwarded
    // exports lazily load it on first use anyway (see version_proxy::get_proc). Must
    // stay out of DllMain: loading it under the loader lock deadlocks startup.
    let _ = init_version_proxy();

    // Self-heal across Discord updates. On update Discord ships a fresh app-X.Y.Z
    // folder and relaunches it via the unhooked Update.exe, so our CreateProcessW hook
    // never copies version.dll there and the bypass dies until a manual reinstall.
    // Squirrel usually downloads the new folder well before the restart, so while this
    // (old) Discord is still running we periodically mirror our files into any app dir
    // that lacks them — the DLL is then already in place when Discord restarts into it.
    loop {
        copy_files_to_all_discord_dirs();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
