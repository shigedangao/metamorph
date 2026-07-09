use super::{CommonClient, UnaryResponse};
use anyhow::Result;
use futures::{Stream, StreamExt};
use reqwest::{Client, ClientBuilder, header::HeaderMap};
use reqwest_streams::{JsonStreamResponse, error::StreamBodyError};
use serde_json::Value;
use std::{
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

/// A wrapper around the `reqwest` HTTP client.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
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
            client: ClientBuilder::new()
                .default_headers(headers)
                .timeout(timeout)
                .build()?,
        }))
    }
}

#[async_trait::async_trait]
impl CommonClient for HttpClient {
    async fn get(&self, url: String) -> Result<UnaryResponse> {
        let start = Instant::now();
        let resp = self.client.get(url).send().await?;

        let elapsed = start.elapsed();
        let status = resp.status().as_u16();
        let body = resp.json::<Value>().await.ok();

        Ok(UnaryResponse {
            duration: elapsed,
            status,
            body,
        })
    }

    async fn post(&self, url: String, body: String) -> Result<UnaryResponse> {
        let start = Instant::now();
        let resp = self.client.post(url).body(body).send().await?;

        let elapsed = start.elapsed();
        let status = resp.status().as_u16();
        let body = resp.json::<Value>().await.ok();

        Ok(UnaryResponse {
            duration: elapsed,
            status,
            body,
        })
    }

    async fn get_stream(
        &self,
        url: String,
        max_payload: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value, StreamBodyError>> + Send>>> {
        let stream = self
            .client
            .get(url)
            .send()
            .await?
            .json_nl_stream::<Value>(max_payload)
            .boxed();

        Ok(stream)
    }

    async fn get_stream_with_body(
        &self,
        url: String,
        body: String,
        max_payload: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value, StreamBodyError>> + Send>>> {
        let stream = self
            .client
            .post(url)
            .body(body.clone())
            .send()
            .await?
            .json_nl_stream::<Value>(max_payload)
            .boxed();

        Ok(stream)
    }
}
