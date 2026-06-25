# http_fo

Rust equivalent of [curl_fo](https://github.com/idrto/curl_fo): client-side HTTP and WebSocket failover over multiple A/AAAA records.

## Features

- TTL-aware DNS LRU cache (default 500 domains)
- TCP connect latency ranking (top 3 IPs by default)
- Parallel TCP race on cold multi-IP connects
- Sequential IP failover on **transport** errors only (not HTTP 4xx/5xx)
- Idempotency header (`X-Curl-FO-Id`) on retried HTTP requests
- WebSocket connect + reconnect rotation (same IP once, then next ranked IP)

## Usage

```rust
use std::sync::Arc;
use http_fo::{Context, Config, HttpClient, RequestSpec, connect_wss, WsReconnectMode, WsReconnectState};
use reqwest::Method;

let mut cfg = Config::default();
cfg.load_env();
let ctx = Arc::new(Context::new(cfg));
let http = HttpClient::new(ctx.clone());

let resp = http.execute(RequestSpec {
    method: Method::GET,
    url: "https://api.example.com/health".into(),
    headers: Default::default(),
    json_body: None,
    is_get_or_head: true,
}).await?;

let mut ws_state = WsReconnectState::default();
let (stream, ip) = connect_wss(
    &ctx,
    "wss://api.example.com/v1/signal",
    Default::default(),
    WsReconnectMode::Initial,
    &mut ws_state,
).await?;
```

## Configuration

Environment variables (also accept `CURL_FO_*` aliases):

| Variable | Default |
|----------|---------|
| `HTTP_FO_LRU_SIZE` | 500 |
| `HTTP_FO_TOP_IPS` | 3 |
| `HTTP_FO_CONNECT_TIMEOUT_MS` | 3000 |
| `HTTP_FO_GET_TIMEOUT_MS` | 15000 |
| `HTTP_FO_OTHER_TIMEOUT_MS` | 60000 |
| `HTTP_FO_DEFAULT_TTL_SEC` | 300 |
| `HTTP_FO_VERBOSE` | off |

## License

MIT
