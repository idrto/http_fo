use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::error::Error;

pub async fn lookup_addresses(host: &str, port: u16) -> Result<Vec<IpAddr>, Error> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|source| Error::Dns {
            host: host.to_string(),
            port,
            source,
        })?
        .collect();
    let mut ips: Vec<IpAddr> = addrs.into_iter().map(|a| a.ip()).collect();
    ips.sort_by_key(|ip| ip.to_string());
    ips.dedup();
    if ips.is_empty() {
        return Err(Error::NoAddresses {
            host: host.to_string(),
            port,
        });
    }
    Ok(ips)
}

/// Rank up to `race_cap` candidate IPs; return the best `top_n` by connect latency.
pub async fn probe_rank(
    port: u16,
    ips: &[IpAddr],
    connect_timeout: Duration,
    race_cap: usize,
    top_n: usize,
    bucket_ms: u64,
    tcp_race: bool,
) -> Vec<IpAddr> {
    debug_assert!(ips.len() > 1);
    let cap = race_cap.max(1).min(ips.len());
    let candidates: Vec<IpAddr> = ips.iter().take(cap).copied().collect();

    if tcp_race {
        if let Some(ranked) = tcp_race_rank(port, &candidates, connect_timeout, top_n, bucket_ms).await
        {
            return ranked;
        }
    }

    sequential_probe_rank(port, &candidates, connect_timeout, top_n, bucket_ms).await
}

async fn sequential_probe_rank(
    port: u16,
    candidates: &[IpAddr],
    connect_timeout: Duration,
    top_n: usize,
    bucket_ms: u64,
) -> Vec<IpAddr> {
    let mut scored = Vec::with_capacity(candidates.len());
    for ip in candidates {
        let (latency, stream) = tcp_connect_probe(*ip, port, connect_timeout).await;
        if let Some(stream) = stream {
            rst_drop(stream);
        }
        let bucket = bucket_latency(latency, bucket_ms);
        scored.push((*ip, bucket, latency));
    }
    sort_and_take(scored, top_n)
}

struct ConnectProbe {
    ip: IpAddr,
    latency: Duration,
    stream: Option<TcpStream>,
}

/// Parallel TCP connect race on at most `candidates.len()` IPs (already capped upstream).
/// Every candidate reports a latency; connected sockets are RST-dropped after probing.
async fn tcp_race_rank(
    port: u16,
    candidates: &[IpAddr],
    connect_timeout: Duration,
    top_n: usize,
    bucket_ms: u64,
) -> Option<Vec<IpAddr>> {
    if candidates.is_empty() {
        return None;
    }

    let (tx, mut rx) = mpsc::channel(candidates.len());
    for &ip in candidates {
        let tx = tx.clone();
        tokio::spawn(async move {
            let probe = tcp_connect_probe(ip, port, connect_timeout).await;
            let _ = tx
                .send(ConnectProbe {
                    ip,
                    latency: probe.0,
                    stream: probe.1,
                })
                .await;
        });
    }
    drop(tx);

    let collect_budget = connect_timeout + Duration::from_millis(250);
    let mut scored = Vec::with_capacity(candidates.len());
    while scored.len() < candidates.len() {
        match timeout(collect_budget, rx.recv()).await {
            Ok(Some(probe)) => {
                if let Some(stream) = probe.stream {
                    rst_drop(stream);
                }
                let bucket = bucket_latency(probe.latency, bucket_ms);
                scored.push((probe.ip, bucket, probe.latency));
            }
            _ => break,
        }
    }

    if scored.is_empty() {
        return None;
    }
    Some(sort_and_take(scored, top_n))
}

async fn tcp_connect_probe(
    ip: IpAddr,
    port: u16,
    connect_timeout: Duration,
) -> (Duration, Option<TcpStream>) {
    let start = std::time::Instant::now();
    let addr = SocketAddr::new(ip, port);
    match timeout(connect_timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => (start.elapsed(), Some(stream)),
        Ok(Err(_)) => (connect_timeout, None),
        Err(_) => (connect_timeout, None),
    }
}

fn bucket_latency(latency: Duration, bucket_ms: u64) -> u64 {
    let bucket = bucket_ms.max(1);
    ((latency.as_millis() as u64 + bucket - 1) / bucket) * bucket
}

fn sort_and_take(mut scored: Vec<(IpAddr, u64, Duration)>, top_n: usize) -> Vec<IpAddr> {
    scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.cmp(&b.2)));
    scored
        .into_iter()
        .map(|(ip, _, _)| ip)
        .take(top_n.max(1))
        .collect()
}

/// Abort a TCP socket with RST (SO_LINGER=0) so the peer resets quickly.
fn rst_drop(stream: TcpStream) {
    if let Ok(std_stream) = stream.into_std() {
        let sock = socket2::SockRef::from(&std_stream);
        let _ = sock.set_linger(Some(Duration::from_secs(0)));
        drop(std_stream);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sequential_probe_respects_top_n() {
        let ips: Vec<IpAddr> = (1..=5).map(|o| IpAddr::from([127, 0, 0, o])).collect();
        let ranked = sequential_probe_rank(
            1,
            &ips,
            Duration::from_millis(50),
            3,
            10,
        )
        .await;
        assert_eq!(ranked.len(), 3);
    }

    #[test]
    fn bucket_rounds_up() {
        assert_eq!(bucket_latency(Duration::from_millis(11), 10), 20);
        assert_eq!(bucket_latency(Duration::from_millis(10), 10), 10);
    }
}
