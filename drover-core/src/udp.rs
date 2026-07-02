use std::ffi::c_void;
use std::path::Path;

use crate::options::PACKET_FILENAME;
use crate::udp_bypass::{apply_voice_bypass, FnSendTo, UdpBypassMode, DEFAULT_QUIC_PACKET};

pub use crate::udp_bypass::{default_packet_bytes, write_default_packet};

pub fn maybe_send_udp_bypass_packets(
    mode: UdpBypassMode,
    process_dir: &Path,
    sendto: FnSendTo,
    sock: usize,
    to: *const c_void,
    to_len: i32,
    first_buffer_len: usize,
) {
    if first_buffer_len != 74 {
        return;
    }

    let packet_path = process_dir.join(PACKET_FILENAME);
    apply_voice_bypass(mode, &packet_path, sendto, sock, to, to_len);
}

/// Bytes to re-send periodically on a live voice socket to keep DPI classifying the flow
/// as the fake QUIC/google traffic. Returns `None` for modes without a persistent fake
/// packet (`None`, `Legacy`) — those have nothing meaningful to repeat.
pub fn keepalive_packet(mode: UdpBypassMode, process_dir: &Path) -> Option<Vec<u8>> {
    match mode {
        UdpBypassMode::None | UdpBypassMode::Legacy => None,
        _ => {
            let packet_path = process_dir.join(PACKET_FILENAME);
            let from_file = std::fs::read(&packet_path)
                .ok()
                .filter(|bytes| !bytes.is_empty());
            Some(from_file.unwrap_or_else(|| DEFAULT_QUIC_PACKET.to_vec()))
        }
    }
}
