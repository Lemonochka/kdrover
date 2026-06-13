#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::path::PathBuf;

fn main() {
    if let Err(message) = run() {
        show_error(&message);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    check_startup_files()?;
    app::run().map_err(|error| error.to_string())
}

fn check_startup_files() -> Result<(), String> {
    let Some(exe_dir) = exe_dir() else {
        return Ok(());
    };

    let stray_version = exe_dir.join("version.dll");
    if stray_version.is_file() {
        return Err(format!(
            "Рядом с kdrover.exe найден файл version.dll:\n{}\n\n\
             Удалите его — он ломает запуск установщика.\n\
             Рядом с kdrover.exe должен быть только kdrover_payload.dll.\n\
             version.dll кладётся только в папку Discord (app-*).",
            stray_version.display()
        ));
    }

    Ok(())
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
}

#[cfg(windows)]
fn show_error(message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let title: Vec<u16> = OsStr::new("KDrover")
        .encode_wide()
        .chain([0])
        .collect();
    let body: Vec<u16> = OsStr::new(message).encode_wide().chain([0]).collect();

    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
            None,
            windows::core::PCWSTR(body.as_ptr()),
            windows::core::PCWSTR(title.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MB_OK
                | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_error(message: &str) {
    eprintln!("{message}");
}
