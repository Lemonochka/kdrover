use std::fs;
use std::path::{Path, PathBuf};

use crate::options::{
    copy_drover_files, load_options, remove_drover_files, save_options, DroverOptions,
    BUILD_DLL_FILENAME, DLL_FILENAME, OPTIONS_FILENAME, PACKET_FILENAME,
};
use crate::udp_bypass::{write_default_packet, UdpBypassMode};
use crate::proxy::ProxyValue;
use crate::find_installed_discord_dirs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProxyMode {
    Http,
    Socks5,
    #[default]
    Direct,
}

#[derive(Debug, Clone, Default)]
pub struct InstallSettings {
    pub mode: ProxyMode,
    pub host: String,
    pub port: String,
    pub auth: bool,
    pub login: String,
    pub password: String,
}

#[derive(Debug)]
pub enum InstallError {
    DiscordRunning,
    DiscordNotFound,
    DllMissing(PathBuf),
    Validation(String),
    Io(std::io::Error),
    PartialFailure { message: String, details: Vec<String> },
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiscordRunning => write!(f, "Please exit Discord before proceeding."),
            Self::DiscordNotFound => write!(f, "The Discord folder was not found."),
            Self::DllMissing(path) => write!(f, "The file '{}' is missing.", path.display()),
            Self::Validation(message) => write!(f, "{message}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::PartialFailure { message, .. } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<std::io::Error> for InstallError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl InstallSettings {
    pub fn from_proxy(proxy: &ProxyValue) -> Self {
        let mode = if !proxy.is_specified {
            ProxyMode::Direct
        } else if proxy.is_socks5 {
            ProxyMode::Socks5
        } else {
            ProxyMode::Http
        };

        Self {
            mode,
            host: proxy.host.clone(),
            port: if proxy.port > 0 {
                proxy.port.to_string()
            } else {
                String::new()
            },
            auth: proxy.is_auth,
            login: proxy.login.clone(),
            password: proxy.password.clone(),
        }
    }

    pub fn proxy_fields_enabled(&self) -> bool {
        self.mode != ProxyMode::Direct
    }

    pub fn auth_fields_enabled(&self) -> bool {
        self.proxy_fields_enabled() && self.auth
    }

    pub fn build_proxy_url(&self) -> Result<String, InstallError> {
        if self.mode == ProxyMode::Direct {
            return Ok(String::new());
        }

        let protocol = match self.mode {
            ProxyMode::Http => "http",
            ProxyMode::Socks5 => "socks5",
            ProxyMode::Direct => return Ok(String::new()),
        };

        let host = self.host.trim();
        if host.is_empty() {
            return Err(InstallError::Validation("Invalid host specified.".into()));
        }

        let port: u16 = self
            .port
            .trim()
            .parse()
            .map_err(|_| InstallError::Validation("Invalid port specified.".into()))?;
        if port == 0 {
            return Err(InstallError::Validation("Invalid port specified.".into()));
        }

        if self.auth {
            if self.mode == ProxyMode::Socks5 {
                return Err(InstallError::Validation(
                    "Authentication for SOCKS5 is not supported in the current version. \
                     Please use an unprotected proxy or switch to HTTP if authentication is required."
                        .into(),
                ));
            }

            let login = self.login.trim();
            let password = self.password.trim();
            if login.is_empty() || password.is_empty() {
                return Err(InstallError::Validation(
                    "Fill in Login and Password or uncheck Authentication.".into(),
                ));
            }

            return Ok(format!("{protocol}://{login}:{password}@{host}:{port}"));
        }

        Ok(format!("{protocol}://{host}:{port}"))
    }

    pub fn to_options(&self) -> Result<DroverOptions, InstallError> {
        Ok(DroverOptions {
            proxy: self.build_proxy_url()?,
            udp_bypass: UdpBypassMode::Auto,
        })
    }
}

pub fn find_settings_source(exe_dir: &Path) -> Option<PathBuf> {
    let dirs = find_installed_discord_dirs();
    if let Some(newest) = newest_discord_dir(&dirs) {
        let path = newest.join(OPTIONS_FILENAME);
        if path.is_file() {
            return Some(path);
        }
    }

    let local = exe_dir.join(OPTIONS_FILENAME);
    if local.is_file() {
        return Some(local);
    }

    None
}

pub fn load_install_settings(exe_dir: &Path) -> InstallSettings {
    if let Some(path) = find_settings_source(exe_dir) {
        let options = load_options(path);
        return InstallSettings::from_proxy(&ProxyValue::parse_from_string(&options.proxy));
    }
    InstallSettings::default()
}

pub fn resolve_dll_path(exe_dir: &Path, override_path: Option<&Path>) -> PathBuf {
    if let Some(path) = override_path {
        return path.to_path_buf();
    }

    let built = exe_dir.join(BUILD_DLL_FILENAME);
    if built.is_file() {
        return built;
    }

    // Dev layout: `cargo run` from workspace root while the DLL sits in target/release.
    if let Ok(cwd) = std::env::current_dir() {
        for base in [cwd.join("target/release"), cwd.join("target/debug")] {
            let candidate = base.join(BUILD_DLL_FILENAME);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    built
}

pub fn install(
    exe_dir: &Path,
    settings: &InstallSettings,
    dll_override: Option<&Path>,
) -> Result<Vec<PathBuf>, InstallError> {
    if is_discord_running() {
        return Err(InstallError::DiscordRunning);
    }

    let dll_path = resolve_dll_path(exe_dir, dll_override);
    if !dll_path.is_file() {
        return Err(InstallError::DllMissing(dll_path));
    }

    let options = settings.to_options()?;
    let targets = find_installed_discord_dirs();
    if targets.is_empty() {
        return Err(InstallError::DiscordNotFound);
    }

    let staging = std::env::temp_dir().join("kdrover-install");
    fs::create_dir_all(&staging)?;
    fs::copy(&dll_path, staging.join(DLL_FILENAME))?;
    save_options(staging.join(OPTIONS_FILENAME), &options)?;
    write_default_packet(&staging.join(PACKET_FILENAME))?;

    let mut installed = Vec::new();
    let mut errors = Vec::new();

    save_options(exe_dir.join(OPTIONS_FILENAME), &options).map_err(|error| errors.push(error.to_string())).ok();

    for dir in targets {
        match copy_drover_files(&staging, &dir) {
            Ok(()) => installed.push(dir),
            Err(error) => errors.push(format!("{}: {error}", dir.display())),
        }
    }

    if !errors.is_empty() {
        return Err(InstallError::PartialFailure {
            message: "Some files could not be written.".into(),
            details: errors,
        });
    }

    Ok(installed)
}

pub fn uninstall() -> Result<Vec<PathBuf>, InstallError> {
    if is_discord_running() {
        return Err(InstallError::DiscordRunning);
    }

    let targets = find_installed_discord_dirs();
    if targets.is_empty() {
        return Err(InstallError::DiscordNotFound);
    }

    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for dir in targets {
        match remove_drover_files(&dir) {
            Ok(()) => removed.push(dir),
            Err(error) => errors.push(format!("{}: {error}", dir.display())),
        }
    }

    if !errors.is_empty() {
        return Err(InstallError::PartialFailure {
            message: "Some files could not be deleted.".into(),
            details: errors,
        });
    }

    Ok(removed)
}

fn newest_discord_dir(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .max_by(|left, right| parse_app_version(left).cmp(&parse_app_version(right)))
        .cloned()
}

fn parse_app_version(path: &Path) -> Vec<u32> {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return Vec::new();
    };

    let Some(version) = name.strip_prefix("app-") else {
        return Vec::new();
    };

    version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

#[cfg(windows)]
pub fn is_discord_running() -> bool {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_err() {
            return false;
        }
        let snapshot = snapshot.unwrap();

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let mut running = false;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe = OsString::from_wide(&entry.szExeFile[..len]);
                if let Some(name) = exe.to_str() {
                    if crate::is_discord_executable(name) {
                        running = true;
                        break;
                    }
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        running
    }
}

#[cfg(not(windows))]
pub fn is_discord_running() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{InstallSettings, ProxyMode};

    #[test]
    fn builds_http_proxy_url() {
        let settings = InstallSettings {
            mode: ProxyMode::Http,
            host: "127.0.0.1".into(),
            port: "8080".into(),
            ..Default::default()
        };
        assert_eq!(
            settings.build_proxy_url().unwrap(),
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn direct_mode_has_empty_proxy() {
        let settings = InstallSettings::default();
        assert_eq!(settings.build_proxy_url().unwrap(), "");
    }
}
