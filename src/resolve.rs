use std::net::IpAddr;
use std::time::Duration;

use crate::cache::DnsCache;
use crate::config::Config;
use crate::error::Error;
use crate::probe::{lookup_addresses, probe_rank};

pub async fn resolve_ranked(
    cache: &DnsCache,
    config: &Config,
    host: &str,
    port: u16,
) -> Result<Vec<IpAddr>, Error> {
    let connect_timeout = Duration::from_millis(config.connect_timeout_ms);
    let ttl = Duration::from_secs(config.default_ttl_sec);

    if let Some(entry) = cache.get(host, port) {
        if config.verbose {
            tracing::debug!(host, port, ips = ?entry.ranked_ips, "http_fo cache hit");
        }
        return Ok(entry.ranked_ips);
    }

    let stale = cache.get_stale(host, port);
    let fresh_ips = lookup_addresses(host, port).await?;

    if let Some(stale) = stale {
        let top = stale.ranked_ips.first().copied();
        if let Some(top_ip) = top {
            if fresh_ips.contains(&top_ip) {
                let ranked: Vec<IpAddr> = stale
                    .ranked_ips
                    .iter()
                    .filter(|ip| fresh_ips.contains(ip))
                    .copied()
                    .collect();
                if !ranked.is_empty() {
                    if config.verbose {
                        tracing::debug!(host, port, "http_fo TTL refresh preserving rank");
                    }
                    cache.put(host, port, fresh_ips, ranked.clone(), ttl);
                    return Ok(ranked);
                }
            }
        }
    }

    let ranked = if fresh_ips.len() <= 1 {
        fresh_ips.clone()
    } else {
        probe_rank(
            host,
            port,
            &fresh_ips,
            connect_timeout,
            config.top_ips,
            config.latency_bucket_ms,
            config.tcp_race,
        )
        .await
    };

    if config.verbose {
        tracing::info!(host, port, ranked = ?ranked, "http_fo resolved and ranked");
    }

    cache.put(host, port, fresh_ips, ranked.clone(), ttl);
    Ok(ranked)
}
