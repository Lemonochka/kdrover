use once_cell::sync::Lazy;
use regex::Regex;

use crate::proxy::ProxyValue;
use crate::socket_manager::SocketManagerItem;

static CONNECT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\ACONNECT ([a-z\d.-]+):(\d+)").expect("valid connect regex"));

pub unsafe fn convert_http_connect_to_socks5(
    item: &SocketManagerItem,
    proxy: &ProxyValue,
    buf: &[u8],
    flags: i32,
    mut send_fn: impl FnMut(usize, *const u8, i32, i32) -> i32,
    mut recv_fn: impl FnMut(usize, *mut u8, i32, i32) -> i32,
) -> bool {
    if !proxy.is_specified || !proxy.is_socks5 || !item.is_tcp {
        return false;
    }

    if buf.len() < 8 || &buf[..8] != b"CONNECT " {
        return false;
    }

    let packet = match std::str::from_utf8(buf) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let Some(caps) = CONNECT_RE.captures(packet) else {
        return false;
    };

    let target_host = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
    let target_port: u16 = caps
        .get(2)
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);

    let sock = item.sock;

    let greeting = [0x05, 0x01, 0x00];
    if (send_fn)(sock, greeting.as_ptr(), greeting.len() as i32, flags) != greeting.len() as i32 {
        return false;
    }

    let mut method = [0u8; 2];
    if (recv_fn)(sock, method.as_mut_ptr(), 2, 0) != 2 || method != [0x05, 0x00] {
        return false;
    }

    let host_bytes = target_host.as_bytes();
    let mut request = Vec::with_capacity(7 + host_bytes.len());
    request.push(0x05);
    request.push(0x01);
    request.push(0x00);
    request.push(0x03);
    request.push(host_bytes.len() as u8);
    request.extend_from_slice(host_bytes);
    request.push((target_port >> 8) as u8);
    request.push((target_port & 0xFF) as u8);

    if (send_fn)(sock, request.as_ptr(), request.len() as i32, flags) != request.len() as i32 {
        return false;
    }

    true
}

pub fn fake_http_connect_response() -> &'static [u8] {
    b"HTTP/1.1 200 Connection Established\r\n\r\n"
}

pub fn try_convert_socks5_reply_to_http(buf: &mut [u8], received: usize) -> Option<usize> {
    if received < 10 {
        return None;
    }
    if &buf[..3] != [0x05, 0x00, 0x00] {
        return None;
    }

    let response = fake_http_connect_response();
    if response.len() > buf.len() {
        return None;
    }

    buf[..response.len()].copy_from_slice(response);
    Some(response.len())
}
