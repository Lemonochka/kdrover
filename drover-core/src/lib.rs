pub mod discord;
pub mod http_auth;
pub mod install;
pub mod options;
pub mod proxy;
pub mod socks5;
pub mod socket_manager;
pub mod udp;
pub mod udp_bypass;

pub use discord::{
    default_discord_roots, dir_has_discord_executable, find_discord_app_dirs,
    find_installed_discord_dirs, is_discord_executable, DISCORD_EXECUTABLES,
};
pub use options::{
    copy_drover_files, extra_filenames, load_options, remove_drover_files, save_options,
    DroverOptions, BUILD_DLL_FILENAME, DLL_FILENAME, OPTIONS_FILENAME, PACKET_FILENAME,
};
pub use udp::{default_packet_bytes, write_default_packet};
pub use udp_bypass::UdpBypassMode;
pub use install::{
    find_settings_source, install, is_discord_running, load_install_settings, resolve_dll_path,
    uninstall, InstallError, InstallSettings, ProxyMode,
};
pub use proxy::ProxyValue;
pub use socket_manager::{SocketManager, SocketManagerItem};
