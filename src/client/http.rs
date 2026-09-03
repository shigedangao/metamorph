use super::{ClientError, CommonClient, UnaryResponse};
use anyhow::Result;
use futures::StreamExt;
use reqwest::{Client, ClientBuilder, header::HeaderMap};
use reqwest_streams::{JsonStreamResponse, error::StreamBodyKind};
use serde_json::Value;
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

/// A wrapper around the `reqwest` HTTP client.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    timeout: Duration,
}

impl HttpClient {
    /// Creates a new `HttpClient` with the given headers and timeout.
    ///
    /// # Arguments
    ///
    /// * `headers` - The headers to set on the client.
    /// * `timeout` - The timeout to set on the client.
    pub fn new(headers: HeaderMap, timeout: Duration) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            client: ClientBuilder::new().default_headers(headers).build()?,
            timeout,
        }))
    }
}

#[async_trait::async_trait]
impl CommonClient for HttpClient {
    async fn get(&self, url: String) -> Result<UnaryResponse, ClientError> {
        let start = Instant::now();
        let resp = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|err| match err.source() {
                Some(source) => ClientError {
                    status: err.status().unwrap_or_default().as_u16(),
                    reason: source.to_string(),
                },
                None => ClientError {
                    status: err.status().unwrap_or_default().as_u16(),
                    reason: err.to_string(),
                },
            })?;

        let elapsed = start.elapsed();
        let status = resp.status().as_u16();
        let body = resp.json::<Value>().await.ok();

        Ok(UnaryResponse {
            duration: elapsed,
            status,
            body,
        })
    }

    async fn post(&self, url: String, body: String) -> Result<UnaryResponse, ClientError> {
        let start = Instant::now();
        let resp = self
            .client
            .post(url)
            .timeout(self.timeout)
            .body(body)
            .send()
            .await
            .map_err(|err| match err.source() {
                Some(source) => ClientError {
                    status: err.status().unwrap_or_default().as_u16(),
                    reason: source.to_string(),
                },
                None => ClientError {
                    status: err.status().unwrap_or_default().as_u16(),
                    reason: err.to_string(),
                },
            })?;

        let elapsed = start.elapsed();
        let status = resp.status().as_u16();
        let body = resp.json::<Value>().await.ok();

        Ok(UnaryResponse {
            duration: elapsed,
            status,
            body,
        })
    }

    async fn get_stream(&self, url: String, max_payload: usize) -> Result<Vec<Value>, ClientError> {
        let mut stream = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|err| ClientError::with_reason(err.to_string()))?
            .json_nl_stream::<Value>(max_payload);

        let mut values = Vec::new();
        while let Some(value) = stream.next().await {
            match value {
                Ok(value) => values.push(value),
                Err(err) => match err.kind() {
                    // Ignore the error as it's due to the stream being closed due to the max length reached.
                    StreamBodyKind::MaxLenReachedError | StreamBodyKind::CodecError => {}
                    StreamBodyKind::InputOutputError => {
                        return Err(ClientError::with_reason(err.to_string()));
                    }
                },
            }
        }

        Ok(values)
    }

    async fn get_stream_with_body(
        &self,
        url: String,
        body: String,
        max_payload: usize,
    ) -> Result<Vec<Value>, ClientError> {
        let mut stream = self
            .client
            .post(url)
            .body(body.clone())
            .send()
            .await
            .map_err(|err| ClientError::with_reason(err.to_string()))?
            .json_nl_stream::<Value>(max_payload);

        let mut values = Vec::new();
        while let Some(value) = stream.next().await {
            match value {
                Ok(value) => values.push(value),
                Err(err) => match err.kind() {
                    // Ignore the error as it's due to the stream being closed due to the max length reached.
                    StreamBodyKind::MaxLenReachedError | StreamBodyKind::CodecError => {}
                    StreamBodyKind::InputOutputError => {
                        return Err(ClientError::with_reason(err.to_string()));
                    }
                },
            }
        }

        Ok(values)
    }
}
