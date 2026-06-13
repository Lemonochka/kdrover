use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::proxy::ProxyValue;
use crate::socket_manager::SocketManagerItem;

pub fn add_http_proxy_authorization_header(
    item: &SocketManagerItem,
    proxy: &ProxyValue,
    buffer: &mut [u8],
) -> bool {
    if !proxy.is_specified || !proxy.is_http || !proxy.is_auth || !item.is_tcp {
        return false;
    }

    let packet = match std::str::from_utf8(buffer) {
        Ok(value) => value,
        Err(_) => return false,
    };

    if packet.contains("\r\nProxy-Authorization: ") {
        return false;
    }

    let Some(ua_start) = packet.find("User-Agent:") else {
        return false;
    };

    let Some(ua_end_rel) = packet[ua_start..].find("\r\n") else {
        return false;
    };
    let ua_end = ua_start + ua_end_rel;
    let ua_len = ua_end - ua_start;

    let credentials = format!("{}:{}", proxy.login, proxy.password);
    let encoded = STANDARD.encode(credentials.as_bytes());
    let mut injected = format!("Proxy-Authorization: Basic {encoded}");

    let filler_len = ua_len.saturating_sub(injected.len());
    if filler_len < 6 {
        return false;
    }

    injected.push_str("\r\nX: ");
    injected.push_str(&"X".repeat(filler_len.saturating_sub(5)));

    if injected.len() != ua_len {
        return false;
    }

    buffer[ua_start..ua_end].copy_from_slice(injected.as_bytes());
    true
}
