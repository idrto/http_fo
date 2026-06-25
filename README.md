# http_fo

Rust equivalent of [curl_fo](https://github.com/idrto/curl_fo): client-side HTTP and WebSocket failover over multiple A/AAAA records.

## Features

- TTL-aware DNS LRU cache (default 500 domains)
- TCP connect latency ranking (top 3 IPs by default)
- Parallel TCP race capped at `race_ips` (default 3) even when DNS returns many addresses
- Losers RST-cancelled after probe; all probe latencies feed the rank order
- Single-IP DNS answers bypass failover entirely (plain connect, no `X-FO-Id`)
- Sequential IP failover on **transport** errors only (not HTTP 4xx/5xx)
- Idempotency header (`X-FO-Id`) on multi-IP HTTP retries
- WebSocket connect + reconnect rotation (same IP once, then next ranked IP)

## Configuration

Environment variables (also accept legacy `CURL_FO_*` aliases where noted):

| Variable | Default |
|----------|---------|
| `HTTP_FO_LRU_SIZE` | 500 |
| `HTTP_FO_TOP_IPS` | 3 |
| `HTTP_FO_RACE_IPS` | 3 |
| `HTTP_FO_CONNECT_TIMEOUT_MS` | 3000 |
| `HTTP_FO_GET_TIMEOUT_MS` | 15000 |
| `HTTP_FO_OTHER_TIMEOUT_MS` | 60000 |
| `HTTP_FO_DEFAULT_TTL_SEC` | 300 |
| `HTTP_FO_VERBOSE` | off |

## License

MIT
