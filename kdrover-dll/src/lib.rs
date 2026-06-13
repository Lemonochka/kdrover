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

            // Forward version APIs synchronously: Discord may query version info early,
            // and loading the real version.dll by full path is loader-lock safe.
            let _ = init_version_proxy();

            // Everything else (option file I/O, LoadLibrary("ws2_32.dll"), detour
            // installation) is forbidden under the loader lock that DllMain holds:
            // LoadLibrary inside DllMain deadlocks the loader. Defer it to a worker
            // thread, which only starts running once DllMain returns and the lock is
            // released.
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
    init_state();
    let _ = install_hooks();
    0
}
