//! Fork policy: unauthenticated OTLP export to a collector on this machine.
//!
//! Upstream's export path requires a cloud-agent dispatch credential
//! (`AuthContext::from_environment`, backed by `WARP_CLOUD_AGENT_OTLP_TOKEN`),
//! which a local user has no way to obtain. This module supplies the missing
//! piece: an [`HttpClient`] that sends OTLP batches with no `Authorization`
//! header and no refresh machinery.
//!
//! It is only ever selected for **loopback** endpoints (see
//! `native::use_local_export`), so removing authentication cannot cause traces
//! to be shipped unauthenticated to a remote host.

use async_compat::Compat;
use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use opentelemetry_http::{HttpClient, HttpError};

/// An OTLP transport for a local collector: no credential, no refresh.
#[derive(Debug, Clone)]
pub(super) struct LocalHttpClient {
    inner: reqwest::Client,
}

impl LocalHttpClient {
    pub(super) fn new() -> Self {
        Self {
            inner: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl HttpClient for LocalHttpClient {
    async fn send_bytes(&self, request: Request<Bytes>) -> Result<Response<Bytes>, HttpError> {
        let request: reqwest::Request = request.try_into()?;
        // Reqwest requires a Tokio-compatible context, while the exporter may
        // run on another executor. Mirrors `AuthenticatedHttpClient`.
        let (status, response) = Compat::new(async {
            let mut response = self.inner.execute(request).await?;
            let status = response.status();
            let response = if status.is_success() {
                let headers = std::mem::take(response.headers_mut());
                Some((headers, response.bytes().await?))
            } else {
                None
            };
            Ok::<_, reqwest::Error>((status, response))
        })
        .await?;

        let Some((headers, body)) = response else {
            return Err(format!("Local OTLP export failed with HTTP {}", status.as_u16()).into());
        };

        let mut response = Response::builder().status(status).body(body)?;
        *response.headers_mut() = headers;
        Ok(response)
    }
}
