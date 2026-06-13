use parking_lot::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct SocketManagerItem {
    pub sock: usize,
    pub is_tcp: bool,
    pub is_udp: bool,
    pub has_sent: bool,
    pub fake_http_proxy_flag: bool,
    pub created_at: Instant,
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
        }
    }
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
        let cutoff = Instant::now() - GARBAGE_AFTER;
        items.retain(|entry| entry.created_at >= cutoff);
    }
}
