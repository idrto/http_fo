use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;
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

pub async fn probe_rank(
    _host: &str,
    port: u16,
    ips: &[IpAddr],
    connect_timeout: Duration,
    top_n: usize,
    bucket_ms: u64,
    tcp_race: bool,
) -> Vec<IpAddr> {
    if ips.len() <= 1 {
        return ips.to_vec();
    }
    if tcp_race && ips.len() > 1 {
        if let Some(winner) = tcp_race_connect(port, ips, connect_timeout).await {
            let mut ranked = vec![winner];
            for ip in ips {
                if *ip != winner {
                    ranked.push(*ip);
                }
            }
            return ranked.into_iter().take(top_n.max(1)).collect();
        }
    }

    let mut scored = Vec::with_capacity(ips.len());
    for ip in ips {
        let latency = tcp_connect_latency(*ip, port, connect_timeout).await;
        let bucket = ((latency.as_millis() as u64 + bucket_ms - 1) / bucket_ms.max(1)) * bucket_ms;
        scored.push((*ip, bucket, latency));
    }
    scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.cmp(&b.2)));
    scored
        .into_iter()
        .map(|(ip, _, _)| ip)
        .take(top_n.max(1))
        .collect()
}

async fn tcp_connect_latency(ip: IpAddr, port: u16, connect_timeout: Duration) -> Duration {
    let start = std::time::Instant::now();
    let addr = SocketAddr::new(ip, port);
    match timeout(connect_timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => start.elapsed(),
        _ => connect_timeout,
    }
}

async fn tcp_race_connect(port: u16, ips: &[IpAddr], connect_timeout: Duration) -> Option<IpAddr> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    for &ip in ips {
        let tx = tx.clone();
        tokio::spawn(async move {
            let addr = SocketAddr::new(ip, port);
            if timeout(connect_timeout, TcpStream::connect(addr))
                .await
                .ok()
                .and_then(|r| r.ok())
                .is_some()
            {
                let _ = tx.send(ip).await;
            }
        });
    }
    drop(tx);
    timeout(connect_timeout, rx.recv())
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single_ip_skips_probe() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let ranked = probe_rank(
            "localhost",
            1,
            &[ip],
            Duration::from_millis(100),
            3,
            10,
            true,
        )
        .await;
        assert_eq!(ranked, vec![ip]);
    }
}
