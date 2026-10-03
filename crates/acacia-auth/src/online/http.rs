use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::SigningKey;
use reqwest::header::HeaderMap;
use reqwest::{RequestBuilder, Response};
use serde::de::DeserializeOwned;

use super::config::AuthConfig;
use super::sign;
use crate::{Error, Result, XboxError};

/// reqwest client plus the server-clock offset Xbox signatures need (go-xsapi `timestamp`):
/// signed requests are rejected when the FILETIME drifts too far from Microsoft's clock.
pub(crate) struct Http {
    pub client: reqwest::Client,
    skew_ms: AtomicI64,
}

impl Http {
    pub fn new(cfg: &AuthConfig) -> Result<Self> {
        let mut builder = reqwest::Client::builder().timeout(cfg.timeout);
        if let Some(proxy) = &cfg.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }
        Ok(Self { client: builder.build()?, skew_ms: AtomicI64::new(0) })
    }

    fn local_ms() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    }

    /// Unix nanoseconds adjusted to the last observed server `Date`.
    pub fn server_now_nanos(&self) -> i64 {
        (Self::local_ms() + self.skew_ms.load(Ordering::Relaxed)) * 1_000_000
    }

    fn observe_date(&self, headers: &HeaderMap) {
        let Some(date) = headers.get("date").and_then(|v| v.to_str().ok()) else { return };
        let Ok(t) = httpdate::parse_http_date(date) else { return };
        let server = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        self.skew_ms.store(server - Self::local_ms(), Ordering::Relaxed);
    }

    /// Sends and decodes a JSON response. Non-2xx responses become [`Error::Xbox`] when they carry
    /// an Xbox `X-Err` code (header or `XErr` body field), else [`Error::Status`].
    pub async fn json<T: DeserializeOwned>(&self, req: RequestBuilder, endpoint: &str) -> Result<T> {
        let resp = self.send(req, endpoint).await?;
        let bytes = resp.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Protocol {
            endpoint: endpoint.into(),
            reason: format!("{e}: {}", truncate(&bytes)),
        })
    }

    pub async fn send(&self, req: RequestBuilder, endpoint: &str) -> Result<Response> {
        let resp = req.send().await?;
        self.observe_date(resp.headers());
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status().as_u16();
        let header_xerr = resp
            .headers()
            .get("x-err")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());
        let body = resp.bytes().await.unwrap_or_default();
        let json = serde_json::from_slice::<serde_json::Value>(&body).ok();
        // Keys only: failed Xbox responses can still carry live tokens.
        let keys: Vec<&str> = json.as_ref().and_then(|v| v.as_object()).map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        tracing::debug!(endpoint, status, x_err = ?header_xerr, ?keys, len = body.len(), "auth request failed");
        let field = |name: &str| json.as_ref().and_then(|v| v.get(name));
        let body_xerr = field("XErr").and_then(serde_json::Value::as_u64).and_then(|c| u32::try_from(c).ok());
        if let Some(code) = header_xerr.or(body_xerr) {
            return Err(Error::Xbox(XboxError::from_code(code)));
        }
        if let Some(url) = field("WebPage").and_then(serde_json::Value::as_str) {
            return Err(Error::ActionRequired { endpoint: endpoint.into(), url: url.into() });
        }
        Err(Error::Status { endpoint: endpoint.into(), status, body: truncate(&body) })
    }

    /// POST `body` with the headers every Xbox auth request carries plus its `Signature`.
    pub fn xbox_post(
        &self,
        url: &str,
        user_agent: &str,
        contract_version: Option<&str>,
        authorization: Option<&str>,
        body: Vec<u8>,
        key: &SigningKey,
    ) -> RequestBuilder {
        let parsed = url::Url::parse(url).expect("static endpoint URL");
        let path = match parsed.query() {
            Some(q) => format!("{}?{q}", parsed.path()),
            None => parsed.path().to_owned(),
        };
        let ft = sign::filetime(self.server_now_nanos());
        let signature =
            sign::signature_header(key, ft, "POST", &path, authorization.unwrap_or(""), &body);
        let mut req = self
            .client
            .post(url)
            .header("User-Agent", user_agent)
            .header("Content-Type", "application/json")
            .header("Signature", signature);
        if let Some(v) = contract_version {
            req = req.header("x-xbl-contract-version", v);
        }
        if let Some(auth) = authorization {
            req = req.header("Authorization", auth);
        }
        req.body(body)
    }
}

fn truncate(body: &[u8]) -> String {
    let s = String::from_utf8_lossy(&body[..body.len().min(512)]);
    s.into_owned()
}
