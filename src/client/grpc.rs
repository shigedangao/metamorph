use super::{CommonClient, UnaryResponse};
use anyhow::{Result, anyhow};
use futures::Stream;
use granc_core::client::{
    DynamicRequest, DynamicResponse, GrancClient, Online, OnlineWithoutReflection,
};
use reqwest::StatusCode;
use reqwest_streams::error::StreamBodyError;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tonic::transport::{ClientTlsConfig, Endpoint};

// Constant for the HTTPS prefix used in gRPC URLs.
const HTTPS_PREFIX: &str = "https://";

#[derive(Debug, Clone)]
enum GrpClientType {
    Online(GrancClient<Online>),
    Offline(GrancClient<OnlineWithoutReflection>),
}

/// A wrapper around the `granc` gRPC client.
#[derive(Debug, Clone)]
pub struct GrpcClient {
    client: GrpClientType,
    headers: Vec<(String, String)>,
}

impl GrpcClient {
    /// Creates a new gRPC client with the specified base URL and headers.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the gRPC service.
    /// * `headers` - The headers to include in gRPC requests.
    pub async fn new(
        base_url: &str,
        headers: Vec<(String, String)>,
        timeout: Duration,
        with_grpc_protoset: Option<PathBuf>,
    ) -> Result<Arc<Self>> {
        let endpoint = match base_url {
            _ if base_url.contains(HTTPS_PREFIX) => Endpoint::from_str(base_url)
                .and_then(|e| e.tls_config(ClientTlsConfig::new().with_native_roots()))
                .map_err(|err| {
                    anyhow!("Unable to create the endpoint with TLS support due to: {err}")
                })?,
            _ => Endpoint::from_str(base_url)?,
        };

        let channel = endpoint
            .timeout(timeout)
            .connect()
            .await
            .map_err(|err| anyhow!("Unable to connect to the rpc server due to {err}"))?;

        let client = GrancClient::from(channel);
        match with_grpc_protoset {
            Some(path) => {
                let descriptor_bytes = tokio::fs::read(path).await?;
                let client_offline = client.with_file_descriptor(descriptor_bytes)?;

                Ok(Arc::new(Self {
                    client: GrpClientType::Offline(client_offline),
                    headers,
                }))
            }
            None => Ok(Arc::new(Self {
                client: GrpClientType::Online(client),
                headers,
            })),
        }
    }

    /// Makes a unary gRPC request to the specified URL with an optional body.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the gRPC service method to call.
    /// * `body` - The optional body of the request as a JSON string.
    pub async fn unary_request(&self, url: &str, body: Option<String>) -> Result<UnaryResponse> {
        let start = Instant::now();
        let Some((service_name, method_name)) = url.split_once("/") else {
            return Err(anyhow::anyhow!(
                "Invalid URL: {} definition for a gRPC request",
                url
            ));
        };

        let json_body = match body {
            Some(b) => serde_json::from_str(&b)?,
            None => json!({}),
        };

        let request = DynamicRequest {
            service: service_name.to_string(),
            method: method_name.to_string(),
            body: json_body,
            headers: self.headers.clone(),
        };

        let response = match &self.client {
            GrpClientType::Online(client) => {
                let mut client_clone = client.clone();
                client_clone.dynamic(request).await?
            }
            GrpClientType::Offline(client) => {
                let mut client_clone = client.clone();
                client_clone.dynamic(request).await?
            }
        };

        //let response = client_clone.dynamic(request).await?;
        if let DynamicResponse::Unary(body) = response {
            let body = body?;

            return Ok(UnaryResponse {
                duration: start.elapsed(),
                status: StatusCode::OK.as_u16(),
                body: Some(body),
            });
        }

        Err(anyhow::anyhow!("Unexpected response type"))
    }
}

#[async_trait::async_trait]
impl CommonClient for GrpcClient {
    async fn get(&self, url: String) -> Result<UnaryResponse> {
        let result = self.unary_request(&url, None).await?;

        Ok(result)
    }

    async fn post(&self, url: String, body: String) -> Result<UnaryResponse> {
        let result = self.unary_request(&url, Some(body)).await?;

        Ok(result)
    }

    async fn get_stream(
        &self,
        _: String,
        _: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value, StreamBodyError>> + Send>>> {
        unimplemented!();
    }

    async fn get_stream_with_body(
        &self,
        _: String,
        _: String,
        _: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Value, StreamBodyError>> + Send>>> {
        unimplemented!();
    }
}
