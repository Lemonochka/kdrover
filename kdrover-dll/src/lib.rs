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
use crate::state::init_state;
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
    // Runs after the loader lock is released, so LoadLibrary is safe here. Resolve the
    // real version.dll first so forwarded exports have a warm handle.
    let _ = init_version_proxy();
    init_state();
    let _ = install_hooks();
    0
}
