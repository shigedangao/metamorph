use anyhow::Result;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::client::grpc::GrpcClient;
use crate::client::http::HttpClient;

pub mod grpc;
pub mod http;

/// Represents the available clients for making HTTP and GRPC requests.
pub enum Clients {
    Http(Arc<http::HttpClient>),
    Grpc(Arc<grpc::GrpcClient>),
}

/// The transport method to use for the client.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportMethod {
    #[default]
    Http,
    Grpc {
        protoset_path: Option<PathBuf>,
    },
}

pub struct UnaryResponse {
    pub duration: Duration,
    pub status: u16,
    pub body: Option<Value>,
}

impl Clients {
    /// Creates a new `Clients` instance based on the transport method and headers.
    ///
    /// # Arguments
    ///
    /// * `ts` - The transport method to use.
    /// * `timeout` - The timeout for the HTTP client in seconds.
    /// * `http_headers` - The HTTP headers to use.
    /// * `grpc_headers` - The GRPC headers to use.
    /// * `grpc_base_url` - The base URL for the GRPC client.
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `Clients` instance.
    pub async fn new(
        ts: TransportMethod,
        timeout: u64,
        http_headers: HeaderMap,
        grpc_headers: Vec<(String, String)>,
        grpc_base_url: String,
    ) -> Result<Self> {
        match ts {
            TransportMethod::Http => {
                let client = HttpClient::new(http_headers, Duration::from_secs(timeout))?;

                Ok(Self::Http(client))
            }
            TransportMethod::Grpc { protoset_path } => {
                let client = GrpcClient::new(
                    &grpc_base_url,
                    grpc_headers,
                    Duration::from_secs(timeout),
                    protoset_path,
                )
                .await?;

                Ok(Self::Grpc(client))
            }
        }
    }

    /// Returns the clients as an `Arc` of the common client trait.
    pub fn get_clients(&self) -> Arc<dyn CommonClient> {
        match self {
            Self::Http(http) => http.clone(),
            Self::Grpc(grpc) => grpc.clone(),
        }
    }
}

/// A trait for common client operations.
///
/// This trait defines the common operations that can be performed on a client,
/// such as GET, POST, and streaming operations.
#[async_trait::async_trait]
pub trait CommonClient: Send + Sync {
    /// Performs a GET request to the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to send the GET request to.
    async fn get(&self, url: String) -> Result<UnaryResponse>;
    /// Performs a POST request to the specified URL with the given body.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to send the POST request to.
    /// * `body` - The body of the POST request.
    async fn post(&self, url: String, body: String) -> Result<UnaryResponse>;
    /// Performs a GET request to the specified URL and returns a stream of results.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to send the GET request to.
    /// * `max_payload` - The maximum payload size to stream.
    async fn get_stream(&self, url: String, max_payload: usize) -> Result<Vec<Value>>;
    /// Performs a GET request to the specified URL with the given body and returns a stream of results.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to send the GET request to.
    /// * `body` - The body of the GET request.
    /// * `max_payload` - The maximum payload size to stream.
    async fn get_stream_with_body(
        &self,
        url: String,
        body: String,
        max_payload: usize,
    ) -> Result<Vec<Value>>;
}
