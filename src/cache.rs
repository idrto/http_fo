use std::net::IpAddr;
use std::time::{Duration, Instant};

use lru::LruCache;
use parking_lot::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub all_ips: Vec<IpAddr>,
    pub ranked_ips: Vec<IpAddr>,
    pub expires_at: Instant,
    pub ws_index: usize,
    pub ws_retried_same: bool,
}

pub struct DnsCache {
    inner: Mutex<LruCache<(String, u16), CacheEntry>>,
}

impl DnsCache {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            inner: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(cap).expect("capacity"),
            )),
        }
    }

    pub fn get(&self, host: &str, port: u16) -> Option<CacheEntry> {
        let mut guard = self.inner.lock();
        let key = (host.to_string(), port);
        let entry = guard.get(&key)?.clone();
        if entry.expires_at <= Instant::now() {
            return None;
        }
        Some(entry)
    }

    pub fn get_stale(&self, host: &str, port: u16) -> Option<CacheEntry> {
        self.inner
            .lock()
            .peek(&(host.to_string(), port))
            .cloned()
    }

    pub fn put(
        &self,
        host: &str,
        port: u16,
        all_ips: Vec<IpAddr>,
        ranked_ips: Vec<IpAddr>,
        ttl: Duration,
    ) {
        let mut guard = self.inner.lock();
        let key = (host.to_string(), port);
        let ws_index = guard.peek(&key).map(|e| e.ws_index).unwrap_or(0);
        let ws_retried_same = guard.peek(&key).map(|e| e.ws_retried_same).unwrap_or(false);
        guard.put(
            key,
            CacheEntry {
                all_ips,
                ranked_ips,
                expires_at: Instant::now() + ttl,
                ws_index,
                ws_retried_same,
            },
        );
    }

    pub fn update_ws_state(&self, host: &str, port: u16, ws_index: usize, ws_retried_same: bool) {
        let mut guard = self.inner.lock();
        let key = (host.to_string(), port);
        if let Some(entry) = guard.get_mut(&key) {
            entry.ws_index = ws_index;
            entry.ws_retried_same = ws_retried_same;
        }
    }

    pub fn invalidate(&self, host: &str, port: u16) {
        self.inner.lock().pop(&(host.to_string(), port));
    }

    pub fn clear(&self) {
        self.inner.lock().clear();
    }
}
