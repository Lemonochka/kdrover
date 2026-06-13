use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Fake QUIC Initial (google.com) from zapret-discord-youtube — bypasses modern DPI
/// that blocks the legacy 0x00/0x01 trick while keeping HTTP proxy for TCP.
pub const DEFAULT_QUIC_PACKET: &[u8] =
    include_bytes!("../assets/quic_initial_www_google_com.bin");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UdpBypassMode {
    /// Use drover-packet.bin if present, otherwise embedded QUIC. No legacy bytes.
    #[default]
    Auto,
    /// Send only the embedded / file QUIC fake packet.
    Quic,
    /// Original drover: 0x00, 0x01 + 50 ms delay.
    Legacy,
    /// QUIC/file packet plus legacy bytes (v0.9 stock behaviour).
    Both,
    /// Disable UDP manipulation (voice only works if UDP is already allowed).
    None,
}

impl UdpBypassMode {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Self::Auto,
            "quic" => Self::Quic,
            "legacy" | "old" => Self::Legacy,
            "both" | "all" | "v09" | "0.9" => Self::Both,
            "none" | "off" | "direct" => Self::None,
            _ => Self::Auto,
        }
    }

    pub fn as_ini_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Quic => "quic",
            Self::Legacy => "legacy",
            Self::Both => "both",
            Self::None => "none",
        }
    }
}

pub type FnSendTo =
    unsafe extern "system" fn(usize, *const u8, i32, i32, *const c_void, i32) -> i32;

pub fn apply_voice_bypass(
    mode: UdpBypassMode,
    packet_path: &Path,
    sendto: FnSendTo,
    sock: usize,
    to: *const c_void,
    to_len: i32,
) {
    match mode {
        UdpBypassMode::None => {}
        UdpBypassMode::Legacy => send_legacy(sendto, sock, to, to_len),
        UdpBypassMode::Quic => send_quic(packet_path, sendto, sock, to, to_len),
        UdpBypassMode::Both => {
            send_quic(packet_path, sendto, sock, to, to_len);
            send_legacy(sendto, sock, to, to_len);
        }
        UdpBypassMode::Auto => {
            if packet_path.is_file() {
                send_file(packet_path, sendto, sock, to, to_len);
            } else {
                send_bytes(DEFAULT_QUIC_PACKET, sendto, sock, to, to_len);
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

pub fn default_packet_bytes() -> &'static [u8] {
    DEFAULT_QUIC_PACKET
}

pub fn write_default_packet(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, DEFAULT_QUIC_PACKET)
}

fn send_quic(packet_path: &Path, sendto: FnSendTo, sock: usize, to: *const c_void, to_len: i32) {
    if packet_path.is_file() {
        send_file(packet_path, sendto, sock, to, to_len);
    } else {
        send_bytes(DEFAULT_QUIC_PACKET, sendto, sock, to, to_len);
    }
    thread::sleep(Duration::from_millis(50));
}

fn send_file(path: &Path, sendto: FnSendTo, sock: usize, to: *const c_void, to_len: i32) {
    if let Ok(data) = fs::read(path) {
        if !data.is_empty() {
            send_bytes(&data, sendto, sock, to, to_len);
        }
    }
}

fn send_bytes(data: &[u8], sendto: FnSendTo, sock: usize, to: *const c_void, to_len: i32) {
    if data.is_empty() {
        return;
    }
    unsafe {
        let _ = sendto(sock, data.as_ptr(), data.len() as i32, 0, to, to_len);
    }
}

fn send_legacy(sendto: FnSendTo, sock: usize, to: *const c_void, to_len: i32) {
    for payload in [0u8, 1u8] {
        unsafe {
            let _ = sendto(sock, &payload as *const u8, 1, 0, to, to_len);
        }
    }
    thread::sleep(Duration::from_millis(50));
}

#[cfg(test)]
mod tests {
    use super::UdpBypassMode;

    #[test]
    fn parses_udp_modes() {
        assert_eq!(UdpBypassMode::parse("quic"), UdpBypassMode::Quic);
        assert_eq!(UdpBypassMode::parse("legacy"), UdpBypassMode::Legacy);
        assert_eq!(UdpBypassMode::parse(""), UdpBypassMode::Auto);
    }
}
