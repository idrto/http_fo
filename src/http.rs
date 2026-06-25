use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, Method};
use uuid::Uuid;

use crate::context::Context;
use crate::endpoint::{parse_endpoint, socket_addr};
use crate::error::Error;
use crate::policy::{should_failover_gateway, should_failover_transport};

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct RequestSpec {
    pub method: Method,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub json_body: Option<serde_json::Value>,
    pub is_get_or_head: bool,
}

/// HTTP client with transport failover across ranked IPs.
pub struct HttpClient {
    ctx: Arc<Context>,
}

impl HttpClient {
    pub fn new(ctx: Arc<Context>) -> Self {
        Self { ctx }
    }

    pub fn context(&self) -> &Arc<Context> {
        &self.ctx
    }

    pub async fn execute(&self, spec: RequestSpec) -> Result<HttpResponse, Error> {
        let endpoint = parse_endpoint(&spec.url)?;
        let resolved = self
            .ctx
            .resolve_ranked(&endpoint.host, endpoint.port)
            .await?;

        let timeout = if spec.is_get_or_head {
            self.ctx.get_timeout()
        } else {
            self.ctx.other_timeout()
        };

        if !resolved.failover_enabled {
            return self
                .execute_direct(&spec, timeout, self.ctx.connect_timeout())
                .await;
        }

        let idempotency = Uuid::new_v4().to_string();
        let idem_header = self.ctx.config().idempotency_header.clone();
        let mut last_transport_err: Option<reqwest::Error> = None;

        for ip in resolved.ranked_ips {
            let client = build_client_for_ip(
                &endpoint.host,
                endpoint.port,
                ip,
                timeout,
                self.ctx.connect_timeout(),
            )?;

            let mut req = client.request(spec.method.clone(), &spec.url);
            for (k, v) in &spec.headers {
                if let (Ok(name), Ok(value)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    req = req.header(name, value);
                }
            }
            if let Ok(name) = HeaderName::from_bytes(idem_header.as_bytes()) {
                if let Ok(value) = HeaderValue::from_str(&idempotency) {
                    req = req.header(name, value);
                }
            }
            if let Some(body) = &spec.json_body {
                req = req
                    .header(CONTENT_TYPE, "application/json")
                    .header(ACCEPT, "application/json")
                    .json(body);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if should_failover_gateway(status, self.ctx.config().failover_gateway) {
                        if self.ctx.config().verbose {
                            tracing::warn!(
                                host = %endpoint.host,
                                ip = %ip,
                                status,
                                "http_fo gateway failover"
                            );
                        }
                        continue;
                    }
                    let body = resp.text().await.unwrap_or_default();
                    return Ok(HttpResponse { status, body });
                }
                Err(e) => {
                    if should_failover_transport(&e) {
                        if self.ctx.config().verbose {
                            tracing::warn!(
                                host = %endpoint.host,
                                ip = %ip,
                                error = %e,
                                "http_fo transport failover"
                            );
                        }
                        last_transport_err = Some(e);
                        continue;
                    }
                    return Err(Error::Http(e));
                }
            }
        }

        if let Some(e) = last_transport_err {
            return Err(Error::Http(e));
        }

        Err(Error::AllIpsFailed {
            host: endpoint.host,
            port: endpoint.port,
        })
    }

    async fn execute_direct(
        &self,
        spec: &RequestSpec,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<HttpResponse, Error> {
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(connect_timeout)
            .build()
            .map_err(Error::Http)?;

        let mut req = client.request(spec.method.clone(), &spec.url);
        for (k, v) in &spec.headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                req = req.header(name, value);
            }
        }
        if let Some(body) = &spec.json_body {
            req = req
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "application/json")
                .json(body);
        }

        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                Ok(HttpResponse { status, body })
            }
            Err(e) => Err(Error::Http(e)),
        }
    }
}

fn build_client_for_ip(
    host: &str,
    port: u16,
    ip: std::net::IpAddr,
    timeout: Duration,
    connect_timeout: Duration,
) -> Result<Client, Error> {
    let resolve_addr = socket_addr(ip, port);
    Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .resolve(host, resolve_addr.parse().map_err(|_| {
            Error::WebSocket(format!("invalid resolve address for {host}"))
        })?)
        .build()
        .map_err(Error::Http)
}
