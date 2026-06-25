//! HTTP and WebSocket client-side failover for multi-homed hostnames.
//!
//! Rust equivalent of [curl_fo](https://github.com/idrto/curl_fo): TTL-aware DNS
//! cache, TCP-connect latency ranking, sequential IP failover on transport errors,
//! and WebSocket reconnect rotation across ranked addresses.

mod cache;
mod config;
mod context;
mod endpoint;
mod error;
mod http;
mod policy;
mod probe;
mod resolve;
mod ws;

pub use config::Config;
pub use context::Context;
pub use endpoint::parse_endpoint;
pub use error::Error;
pub use http::{HttpClient, HttpResponse, RequestSpec};
pub use policy::{should_failover_gateway, should_failover_transport};
pub use ws::{connect_wss, prepare_ws_reconnect, WsReconnectMode, WsReconnectState};
