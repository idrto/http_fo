use std::net::IpAddr;
use std::time::Duration;

use http::HeaderMap;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::context::Context;
use crate::endpoint::{parse_endpoint, Scheme};
use crate::error::Error;

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsReconnectMode {
    /// Try ranked IPs sequentially (initial connect).
    Initial,
    /// Try one IP per call using curl_fo reconnect index (call [`prepare_ws_reconnect`] first).
    AfterDisconnect,
}

#[derive(Debug, Clone, Default)]
pub struct WsReconnectState {
    pub connected_index: usize,
    pub retried_same: bool,
}

impl WsReconnectState {
    pub fn reset(&mut self) {
        self.connected_index = 0;
        self.retried_same = false;
    }
}

/// Advance state before a reconnect attempt (curl_fo: same IP once, then next ranked IP).
pub fn prepare_ws_reconnect(state: &mut WsReconnectState, ranked_len: usize) {
    if ranked_len == 0 {
        return;
    }
    if state.retried_same {
        state.connected_index = (state.connected_index + 1) % ranked_len;
        state.retried_same = false;
    } else {
        state.retried_same = true;
    }
}

/// Connect a WebSocket with IP failover (bypassed when DNS returns a single address).
pub async fn connect_wss(
    ctx: &Context,
    url: &str,
    extra_headers: HeaderMap,
    mode: WsReconnectMode,
    state: &mut WsReconnectState,
) -> Result<(WsStream, IpAddr), Error> {
    let endpoint = parse_endpoint(url)?;
    let resolved = ctx.resolve_ranked(&endpoint.host, endpoint.port).await?;

    if !resolved.failover_enabled {
        let ip = resolved
            .ranked_ips
            .first()
            .copied()
            .ok_or_else(|| Error::NoAddresses {
                host: endpoint.host.clone(),
                port: endpoint.port,
            })?;
        let stream = connect_wss_direct(url, extra_headers).await?;
        state.reset();
        return Ok((stream, ip));
    }

    let ranked = resolved.ranked_ips;
    let ips_to_try = ips_for_mode(&ranked, mode, state);

    if ips_to_try.is_empty() {
        return Err(Error::AllIpsFailed {
            host: endpoint.host.clone(),
            port: endpoint.port,
        });
    }

    let connect_timeout = ctx.connect_timeout();
    let mut last_err = String::new();

    for ip in ips_to_try {
        match connect_wss_to_ip(
            url,
            &endpoint.host,
            endpoint.port,
            endpoint.scheme,
            ip,
            &extra_headers,
            connect_timeout,
        )
        .await
        {
            Ok(stream) => {
                if mode == WsReconnectMode::Initial {
                    if let Some(idx) = ranked.iter().position(|r| r == &ip) {
                        state.connected_index = idx;
                    }
                    state.retried_same = false;
                }
                ctx.update_ws_state(
                    &endpoint.host,
                    endpoint.port,
                    state.connected_index,
                    state.retried_same,
                );
                if ctx.config().verbose {
                    tracing::info!(
                        host = %endpoint.host,
                        ip = %ip,
                        mode = ?mode,
                        "http_fo websocket connected"
                    );
                }
                return Ok((stream, ip));
            }
            Err(e) => {
                if ctx.config().verbose {
                    tracing::warn!(
                        host = %endpoint.host,
                        ip = %ip,
                        error = %e,
                        "http_fo websocket connect failed"
                    );
                }
                last_err = e.to_string();
            }
        }
    }

    Err(Error::WebSocket(if last_err.is_empty() {
        format!("all IPs failed for {}:{}", endpoint.host, endpoint.port)
    } else {
        last_err
    }))
}

fn ips_for_mode(ranked: &[IpAddr], mode: WsReconnectMode, state: &WsReconnectState) -> Vec<IpAddr> {
    if ranked.is_empty() {
        return Vec::new();
    }
    match mode {
        WsReconnectMode::Initial => ranked.to_vec(),
        WsReconnectMode::AfterDisconnect => {
            let idx = state.connected_index.min(ranked.len() - 1);
            ranked.get(idx).copied().into_iter().collect()
        }
    }
}

async fn connect_wss_to_ip(
    url: &str,
    host: &str,
    port: u16,
    scheme: Scheme,
    ip: IpAddr,
    extra_headers: &HeaderMap,
    connect_timeout: Duration,
) -> Result<WsStream, Error> {
    let mut request = url
        .into_client_request()
        .map_err(|e| Error::WebSocket(e.to_string()))?;
    for (k, v) in extra_headers.iter() {
        request.headers_mut().insert(k.clone(), v.clone());
    }

    if !scheme.is_tls() {
        let addr = (ip, port);
        let tcp = timeout(connect_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| Error::WebSocket("connect timeout".into()))?
            .map_err(|e| Error::WebSocket(e.to_string()))?;
        let (stream, _) = tokio_tungstenite::client_async(request, MaybeTlsStream::Plain(tcp))
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))?;
        return Ok(stream);
    }

    let addr = (ip, port);
    let tcp = timeout(connect_timeout, TcpStream::connect(addr))
        .await
        .map_err(|_| Error::WebSocket("connect timeout".into()))?
        .map_err(|e| Error::WebSocket(e.to_string()))?;
    tcp.set_nodelay(true).ok();

    let tls_connector =
        native_tls::TlsConnector::new().map_err(|e| Error::WebSocket(e.to_string()))?;
    let tls_connector = tokio_native_tls::TlsConnector::from(tls_connector);
    let tls = timeout(connect_timeout, tls_connector.connect(host, tcp))
        .await
        .map_err(|_| Error::WebSocket("tls timeout".into()))?
        .map_err(|e| Error::WebSocket(e.to_string()))?;

    let (stream, _) = tokio_tungstenite::client_async(request, MaybeTlsStream::NativeTls(tls))
        .await
        .map_err(|e| Error::WebSocket(e.to_string()))?;
    Ok(stream)
}

/// Plain hostname connect (single-IP bypass / tests).
pub async fn connect_wss_direct(url: &str, extra_headers: HeaderMap) -> Result<WsStream, Error> {
    let mut request = url
        .into_client_request()
        .map_err(|e| Error::WebSocket(e.to_string()))?;
    for (k, v) in extra_headers.iter() {
        request.headers_mut().insert(k.clone(), v.clone());
    }
    connect_async(request)
        .await
        .map(|(s, _)| s)
        .map_err(|e| Error::WebSocket(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_mode_tries_all_ranked() {
        let ips: Vec<IpAddr> = ["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()].to_vec();
        let state = WsReconnectState::default();
        let try_list = ips_for_mode(&ips, WsReconnectMode::Initial, &state);
        assert_eq!(try_list.len(), 2);
    }

    #[test]
    fn reconnect_targets_current_index() {
        let ips: Vec<IpAddr> = ["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()].to_vec();
        let state = WsReconnectState {
            connected_index: 1,
            retried_same: true,
        };
        let try_list = ips_for_mode(&ips, WsReconnectMode::AfterDisconnect, &state);
        assert_eq!(try_list, vec![ips[1]]);
    }

    #[test]
    fn prepare_advances_after_same_ip_retry() {
        let mut state = WsReconnectState {
            connected_index: 0,
            retried_same: true,
        };
        prepare_ws_reconnect(&mut state, 3);
        assert_eq!(state.connected_index, 1);
        assert!(!state.retried_same);
    }
}
