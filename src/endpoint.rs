use std::net::IpAddr;

use url::Url;

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    Ws,
    Wss,
}

impl Scheme {
    pub fn default_port(self) -> u16 {
        match self {
            Self::Http | Self::Ws => 80,
            Self::Https | Self::Wss => 443,
        }
    }

    pub fn is_tls(self) -> bool {
        matches!(self, Self::Https | Self::Wss)
    }
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub scheme: Scheme,
    pub origin: String,
}

pub fn parse_endpoint(url: &str) -> Result<Endpoint, Error> {
    let parsed = Url::parse(url)?;
    let scheme = match parsed.scheme() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        "ws" => Scheme::Ws,
        "wss" => Scheme::Wss,
        other => {
            return Err(Error::WebSocket(format!("unsupported scheme: {other}")));
        }
    };
    let host = parsed
        .host_str()
        .ok_or_else(|| Error::WebSocket("URL missing host".into()))?
        .to_string();
    let port = parsed.port().unwrap_or_else(|| scheme.default_port());
    Ok(Endpoint {
        host,
        port,
        scheme,
        origin: parsed.origin().ascii_serialization(),
    })
}

pub fn socket_addr(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(_) => format!("{ip}:{port}"),
        IpAddr::V6(_) => format!("[{ip}]:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wss_default_port() {
        let ep = parse_endpoint("wss://idr.to/v1/signal").unwrap();
        assert_eq!(ep.host, "idr.to");
        assert_eq!(ep.port, 443);
        assert!(ep.scheme.is_tls());
    }

    #[test]
    fn formats_ipv6_socket() {
        let ip: IpAddr = "2001:db8::1".parse().unwrap();
        assert_eq!(socket_addr(ip, 443), "[2001:db8::1]:443");
    }
}
