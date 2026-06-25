use std::net::IpAddr;
use std::time::Duration;

use crate::cache::DnsCache;
use crate::config::Config;
use crate::error::Error;
use crate::probe::{lookup_addresses, probe_rank};

/// Result of resolving a host for failover-aware connect.
#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub ranked_ips: Vec<IpAddr>,
    /// When false, callers should use a plain connect (no IP pinning, no retry loop).
    pub failover_enabled: bool,
}

pub async fn resolve_ranked(
    cache: &DnsCache,
    config: &Config,
    host: &str,
    port: u16,
) -> Result<ResolveResult, Error> {
    let connect_timeout = Duration::from_millis(config.connect_timeout_ms);
    let ttl = Duration::from_secs(config.default_ttl_sec);

    if let Some(entry) = cache.get(host, port) {
        if config.verbose {
            tracing::debug!(
                host,
                port,
                ips = ?entry.ranked_ips,
                failover = entry.failover_enabled,
                "http_fo cache hit"
            );
        }
        return Ok(ResolveResult {
            ranked_ips: entry.ranked_ips,
            failover_enabled: entry.failover_enabled,
        });
    }

    let stale = cache.get_stale(host, port);
    let fresh_ips = lookup_addresses(host, port).await?;

    if fresh_ips.len() <= 1 {
        let ranked = fresh_ips.clone();
        let result = ResolveResult {
            ranked_ips: ranked.clone(),
            failover_enabled: false,
        };
        if config.verbose {
            tracing::debug!(host, port, ip = ?ranked.first(), "http_fo single IP bypass");
        }
        cache.put(host, port, fresh_ips, ranked, false, ttl);
        return Ok(result);
    }

    if let Some(stale) = stale {
        if stale.failover_enabled {
            let top = stale.ranked_ips.first().copied();
            if let Some(top_ip) = top {
                if fresh_ips.contains(&top_ip) {
                    let ranked: Vec<IpAddr> = stale
                        .ranked_ips
                        .iter()
                        .filter(|ip| fresh_ips.contains(ip))
                        .copied()
                        .collect();
                    if ranked.len() > 1 {
                        if config.verbose {
                            tracing::debug!(host, port, "http_fo TTL refresh preserving rank");
                        }
                        cache.put(host, port, fresh_ips, ranked.clone(), true, ttl);
                        return Ok(ResolveResult {
                            ranked_ips: ranked,
                            failover_enabled: true,
                        });
                    }
                }
            }
        }
    }

    let ranked = probe_rank(
        port,
        &fresh_ips,
        connect_timeout,
        config.race_ips,
        config.top_ips,
        config.latency_bucket_ms,
        config.tcp_race,
    )
    .await;

    let failover_enabled = ranked.len() > 1;
    if config.verbose {
        tracing::info!(
            host,
            port,
            ranked = ?ranked,
            failover = failover_enabled,
            "http_fo resolved and ranked"
        );
    }

    cache.put(
        host,
        port,
        fresh_ips,
        ranked.clone(),
        failover_enabled,
        ttl,
    );
    Ok(ResolveResult {
        ranked_ips: ranked,
        failover_enabled,
    })
}
