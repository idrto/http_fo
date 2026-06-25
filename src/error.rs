use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("dns resolution failed for {host}:{port}: {source}")]
    Dns {
        host: String,
        port: u16,
        source: std::io::Error,
    },
    #[error("no addresses for {host}:{port}")]
    NoAddresses { host: String, port: u16 },
    #[error("all IPs failed for {host}:{port}")]
    AllIpsFailed { host: String, port: u16 },
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("websocket connect failed: {0}")]
    WebSocket(String),
    #[error("invalid header name")]
    InvalidHeader,
}
