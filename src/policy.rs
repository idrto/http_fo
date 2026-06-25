/// Returns true when another ranked IP should be tried for HTTP.
pub fn should_failover_transport(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_body() || err.is_decode()
}

/// Optional gateway failover (502/503/504) when enabled in config.
pub fn should_failover_gateway(status: u16, enabled: bool) -> bool {
    enabled && matches!(status, 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_codes_only_when_enabled() {
        assert!(!should_failover_gateway(503, false));
        assert!(should_failover_gateway(503, true));
        assert!(!should_failover_gateway(500, true));
    }
}
