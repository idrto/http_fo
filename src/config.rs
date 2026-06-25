use std::env;

/// Failover configuration (defaults match curl_fo).
#[derive(Debug, Clone)]
pub struct Config {
    pub lru_capacity: usize,
    pub top_ips: usize,
    pub get_timeout_ms: u64,
    pub other_timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub idempotency_header: String,
    pub latency_bucket_ms: u64,
    pub default_ttl_sec: u64,
    pub verbose: bool,
    pub failover_gateway: bool,
    pub tcp_race: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lru_capacity: 500,
            top_ips: 3,
            get_timeout_ms: 15_000,
            other_timeout_ms: 60_000,
            connect_timeout_ms: 3_000,
            idempotency_header: "X-Curl-FO-Id".into(),
            latency_bucket_ms: 10,
            default_ttl_sec: 300,
            verbose: false,
            failover_gateway: false,
            tcp_race: true,
        }
    }
}

impl Config {
    pub fn load_env(&mut self) {
        if let Ok(v) = env::var("HTTP_FO_LRU_SIZE").or_else(|_| env::var("CURL_FO_LRU_SIZE")) {
            if let Ok(n) = v.parse() {
                self.lru_capacity = n;
            }
        }
        if let Ok(v) = env::var("HTTP_FO_TOP_IPS").or_else(|_| env::var("CURL_FO_TOP_IPS")) {
            if let Ok(n) = v.parse() {
                self.top_ips = n;
            }
        }
        if let Ok(v) =
            env::var("HTTP_FO_GET_TIMEOUT_MS").or_else(|_| env::var("CURL_FO_GET_TIMEOUT_MS"))
        {
            if let Ok(n) = v.parse() {
                self.get_timeout_ms = n;
            }
        }
        if let Ok(v) =
            env::var("HTTP_FO_OTHER_TIMEOUT_MS").or_else(|_| env::var("CURL_FO_OTHER_TIMEOUT_MS"))
        {
            if let Ok(n) = v.parse() {
                self.other_timeout_ms = n;
            }
        }
        if let Ok(v) = env::var("HTTP_FO_CONNECT_TIMEOUT_MS")
            .or_else(|_| env::var("CURL_FO_CONNECT_TIMEOUT_MS"))
        {
            if let Ok(n) = v.parse() {
                self.connect_timeout_ms = n;
            }
        }
        if let Ok(v) = env::var("HTTP_FO_IDEMPOTENCY_HEADER")
            .or_else(|_| env::var("CURL_FO_IDEMPOTENCY_HEADER"))
        {
            if !v.is_empty() {
                self.idempotency_header = v;
            }
        }
        if let Ok(v) = env::var("HTTP_FO_LATENCY_BUCKET_MS")
            .or_else(|_| env::var("CURL_FO_LATENCY_BUCKET_MS"))
        {
            if let Ok(n) = v.parse() {
                self.latency_bucket_ms = n;
            }
        }
        if let Ok(v) =
            env::var("HTTP_FO_DEFAULT_TTL_SEC").or_else(|_| env::var("CURL_FO_DEFAULT_TTL_SEC"))
        {
            if let Ok(n) = v.parse() {
                self.default_ttl_sec = n;
            }
        }
        if let Ok(v) = env::var("HTTP_FO_VERBOSE").or_else(|_| env::var("CURL_FO_VERBOSE")) {
            self.verbose = matches!(v.as_str(), "1" | "true" | "yes" | "on");
        }
        if let Ok(v) = env::var("HTTP_FO_FAILOVER_GATEWAY")
            .or_else(|_| env::var("CURL_FO_FAILOVER_GATEWAY"))
        {
            self.failover_gateway = matches!(v.as_str(), "1" | "true" | "yes" | "on");
        }
        if let Ok(v) = env::var("HTTP_FO_TCP_RACE").or_else(|_| env::var("CURL_FO_TCP_RACE")) {
            if matches!(v.as_str(), "0" | "false" | "no" | "off") {
                self.tcp_race = false;
            }
        }
    }

    pub fn with_network(
        lru_capacity: usize,
        top_ips: usize,
        connect_timeout_ms: u64,
        get_timeout_ms: u64,
        other_timeout_ms: u64,
    ) -> Self {
        let mut cfg = Self::default();
        if lru_capacity > 0 {
            cfg.lru_capacity = lru_capacity;
        }
        if top_ips > 0 {
            cfg.top_ips = top_ips;
        }
        if connect_timeout_ms > 0 {
            cfg.connect_timeout_ms = connect_timeout_ms;
        }
        if get_timeout_ms > 0 {
            cfg.get_timeout_ms = get_timeout_ms;
        }
        if other_timeout_ms > 0 {
            cfg.other_timeout_ms = other_timeout_ms;
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_curl_fo() {
        let cfg = Config::default();
        assert_eq!(cfg.lru_capacity, 500);
        assert_eq!(cfg.top_ips, 3);
        assert_eq!(cfg.connect_timeout_ms, 3_000);
        assert_eq!(cfg.idempotency_header, "X-Curl-FO-Id");
    }
}
