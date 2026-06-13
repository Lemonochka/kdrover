use std::ffi::c_void;
use std::path::Path;

use crate::options::PACKET_FILENAME;
use crate::udp_bypass::{apply_voice_bypass, FnSendTo, UdpBypassMode};

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
