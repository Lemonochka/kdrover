use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// Widest sockaddr we cache for the voice keepalive (sockaddr_in6 is 28 bytes).
pub const SOCKADDR_MAX: usize = 32;

#[derive(Debug, Clone, Copy)]
pub struct SocketManagerItem {
    pub sock: usize,
    pub is_tcp: bool,
    pub is_udp: bool,
    pub has_sent: bool,
    pub fake_http_proxy_flag: bool,
    pub created_at: Instant,
    /// Set once the one-time UDP voice bypass has fired for this socket.
    pub is_voice: bool,
    /// Destination captured from the voice discovery `sendto`, reused by the keepalive.
    pub dest: [u8; SOCKADDR_MAX],
    pub dest_len: i32,
}

impl Default for SocketManagerItem {
    fn default() -> Self {
        Self {
            sock: 0,
            is_tcp: false,
            is_udp: false,
            has_sent: false,
            fake_http_proxy_flag: false,
            created_at: Instant::now(),
            is_voice: false,
            dest: [0; SOCKADDR_MAX],
            dest_len: 0,
        }
    }
}

/// A live voice socket plus the destination its fake keepalive packet should target.
#[derive(Debug, Clone, Copy)]
pub struct VoiceTarget {
    pub sock: usize,
    pub dest: [u8; SOCKADDR_MAX],
    pub dest_len: i32,
}

pub struct SocketManager {
    items: Mutex<Vec<SocketManagerItem>>,
}

const GARBAGE_AFTER: Duration = Duration::from_secs(30);

impl Default for SocketManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SocketManager {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
        }
    }

    pub fn add(&self, sock: usize, sock_type: i32, sock_protocol: i32) {
        const SOCK_STREAM: i32 = 1;
        const SOCK_DGRAM: i32 = 2;
        const IPPROTO_TCP: i32 = 6;
        const IPPROTO_UDP: i32 = 17;

        let item = SocketManagerItem {
            sock,
            is_tcp: sock_type == SOCK_STREAM && (sock_protocol == IPPROTO_TCP || sock_protocol == 0),
            is_udp: sock_type == SOCK_DGRAM && (sock_protocol == IPPROTO_UDP || sock_protocol == 0),
            has_sent: false,
            fake_http_proxy_flag: false,
            created_at: Instant::now(),
            is_voice: false,
            dest: [0; SOCKADDR_MAX],
            dest_len: 0,
        };

        let mut items = self.items.lock();
        Self::collect_garbage(&mut items);

        if let Some(existing) = items.iter_mut().find(|entry| entry.sock == sock) {
            *existing = item;
        } else {
            items.push(item);
        }
    }

    pub fn is_first_send(&self, sock: usize) -> Option<SocketManagerItem> {
        let mut items = self.items.lock();
        let index = items.iter().position(|entry| entry.sock == sock)?;
        if items[index].has_sent {
            return None;
        }
        items[index].has_sent = true;
        Some(items[index])
    }

    /// Returns `true` exactly once per UDP socket — on the first voice discovery packet
    /// we recognise — and records the destination so the keepalive can reuse it. Keyed on
    /// the 74-byte discovery packet rather than "first send of any size" so an unrelated
    /// earlier datagram (STUN, a stray keepalive) can't consume the trigger and leave the
    /// real discovery packet un-bypassed — which is exactly when DPI throttles the flow.
    pub fn mark_udp_bypass(&self, sock: usize, dest: &[u8]) -> bool {
        let mut items = self.items.lock();
        let Some(entry) = items.iter_mut().find(|entry| entry.sock == sock) else {
            return false;
        };
        if !entry.is_udp || entry.is_voice {
            return false;
        }
        entry.is_voice = true;
        let len = dest.len().min(SOCKADDR_MAX);
        entry.dest[..len].copy_from_slice(&dest[..len]);
        entry.dest_len = len as i32;
        true
    }

    pub fn voice_targets(&self) -> Vec<VoiceTarget> {
        self.items
            .lock()
            .iter()
            .filter(|entry| entry.is_voice && entry.dest_len > 0)
            .map(|entry| VoiceTarget {
                sock: entry.sock,
                dest: entry.dest,
                dest_len: entry.dest_len,
            })
            .collect()
    }

    pub fn remove(&self, sock: usize) {
        self.items.lock().retain(|entry| entry.sock != sock);
    }

    pub fn set_fake_http_proxy_flag(&self, sock: usize) {
        let mut items = self.items.lock();
        if let Some(entry) = items.iter_mut().find(|entry| entry.sock == sock) {
            entry.fake_http_proxy_flag = true;
        }
    }

    pub fn reset_fake_http_proxy_flag(&self, sock: usize) -> bool {
        let mut items = self.items.lock();
        let Some(entry) = items.iter_mut().find(|entry| entry.sock == sock) else {
            return false;
        };
        if !entry.fake_http_proxy_flag {
            return false;
        }
        entry.fake_http_proxy_flag = false;
        true
    }

    fn collect_garbage(items: &mut Vec<SocketManagerItem>) {
        // Keep voice sockets regardless of age: a call outlives the 30 s window and the
        // keepalive must keep targeting it. Dead voice entries are pruned by the keepalive
        // (on WSAENOTSOCK) or overwritten when their handle is reused by a new socket().
        let cutoff = Instant::now() - GARBAGE_AFTER;
        items.retain(|entry| entry.is_voice || entry.created_at >= cutoff);
    }
}
