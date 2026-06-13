use std::fs;
use std::path::{Path, PathBuf};

pub const DISCORD_EXECUTABLES: [&str; 3] = ["Discord.exe", "DiscordCanary.exe", "DiscordPTB.exe"];

pub fn is_discord_executable(filename: &str) -> bool {
    DISCORD_EXECUTABLES
        .iter()
        .any(|name| name.eq_ignore_ascii_case(filename))
}

pub fn dir_has_discord_executable(dir: &Path) -> bool {
    DISCORD_EXECUTABLES
        .iter()
        .any(|name| dir.join(name).is_file())
}

pub fn find_discord_app_dirs(base_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(base_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("app-"))
                && dir_has_discord_executable(path)
        })
        .collect()
}

pub fn default_discord_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        for name in ["Discord", "DiscordCanary", "DiscordPTB"] {
            roots.push(PathBuf::from(&local_app_data).join(name));
        }
    }
    roots
}

pub fn find_installed_discord_dirs() -> Vec<PathBuf> {
    default_discord_roots()
        .into_iter()
        .flat_map(|root| find_discord_app_dirs(&root))
        .collect()
}
