use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::DnsCache;
use crate::config::Config;
use crate::error::Error;
use crate::resolve::resolve_ranked;

/// Shared DNS cache and failover configuration.
pub struct Context {
    config: Config,
    cache: DnsCache,
}

impl Context {
    pub fn new(config: Config) -> Self {
        let capacity = config.lru_capacity;
        Self {
            config,
            cache: DnsCache::new(capacity),
        }
    }

    pub fn with_defaults() -> Self {
        let mut config = Config::default();
        config.load_env();
        Self::new(config)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn invalidate(&self, host: &str, port: u16) {
        self.cache.invalidate(host, port);
    }

    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    pub async fn resolve_ranked(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, Error> {
        resolve_ranked(&self.cache, &self.config, host, port).await
    }

    pub fn connect_timeout(&self) -> Duration {
        Duration::from_millis(self.config.connect_timeout_ms)
    }

    pub fn get_timeout(&self) -> Duration {
        Duration::from_millis(self.config.get_timeout_ms)
    }

    pub fn other_timeout(&self) -> Duration {
        Duration::from_millis(self.config.other_timeout_ms)
    }

    pub fn update_ws_state(&self, host: &str, port: u16, ws_index: usize, ws_retried_same: bool) {
        self.cache
            .update_ws_state(host, port, ws_index, ws_retried_same);
    }

    pub fn ws_state(&self, host: &str, port: u16) -> (usize, bool) {
        self.cache
            .get(host, port)
            .or_else(|| self.cache.get_stale(host, port))
            .map(|e| (e.ws_index, e.ws_retried_same))
            .unwrap_or((0, false))
    }
}

pub fn shared(config: Config) -> Arc<Context> {
    Arc::new(Context::new(config))
}
