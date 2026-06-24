use crate::endpoints::params::{Endpoint, SupportedMethod};
use anyhow::Result;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use reqwest_streams::{JsonStreamResponse, error::StreamBodyKind};
use serde_json::Value;
use std::time::Instant;

/// Represents a component of a client endpoint, including the URL, path, method, and body.
#[derive(Debug, Clone)]
pub struct ClientEndpointComponent {
    url: String,
    pub check_path: Option<String>,
    pub reconcile_path: Option<String>,
    method: SupportedMethod,
    stream: bool,
    body: Option<String>,
}

/// Represents the output of a client endpoint request, including the elapsed time, status code, and nodes.
#[derive(Debug, Clone, Default)]
pub struct ClientEndpointOutput {
    pub elapsed: u128,
    pub status: StatusCode,
    pub nodes: Option<Vec<Value>>,
    pub reconcile_nodes: Option<Vec<Value>>,
}

impl ClientEndpointComponent {
    /// Creates a new `ClientEndpointComponent` with the given URL, endpoint parameters, stream flag, and body.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the endpoint to send the request to.
    /// * `endpoint_param` - The endpoint parameters to use for the request.
    /// * `stream` - Whether the request should be streamed or not.
    /// * `body` - The body of the request, if any.
    pub fn new(url: String, endpoint_param: Endpoint, stream: bool, body: Option<String>) -> Self {
        Self {
            url,
            check_path: endpoint_param.check_path,
            reconcile_path: endpoint_param.reconcile_path,
            method: endpoint_param.method,
            stream,
            body,
        }
    }

    /// Sends the request to the endpoint and returns the response as a `ClientEndpointOutput`.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for the request.
    pub async fn send(&self, client: &Client) -> Result<ClientEndpointOutput> {
        match self.stream {
            true => self.run_stream_request(client).await,
            false => self.run_unary_request(client).await,
        }
    }

    /// Sends the request to the endpoint and returns the response as a `ClientEndpointOutput`.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for the request.
    /// * `max_payload_size` - The maximum payload size to use for the request.
    async fn run_unary_request(&self, client: &Client) -> Result<ClientEndpointOutput> {
        let start = Instant::now();

        // Send the request and get the response
        let response = match self.method {
            SupportedMethod::Get => client.get(&self.url).send().await?,
            SupportedMethod::Post => {
                client
                    .post(&self.url)
                    .body(self.body.clone().unwrap_or_default())
                    .send()
                    .await?
            }
        };

        let elapsed = start.elapsed();
        // Get the status from the response
        let status = response.status();

        if let Some(check_path) = &self.check_path {
            let path = serde_json_path::JsonPath::parse(check_path)?;
            let body = response.json::<Value>().await?;

            let node = path.query(&body).exactly_one().unwrap_or_default();

            return Ok(ClientEndpointOutput {
                elapsed: elapsed.as_millis(),
                status,
                nodes: Some(vec![node.clone()]),
                reconcile_nodes: None,
            });
        }

        Ok(ClientEndpointOutput {
            elapsed: elapsed.as_millis(),
            status,
            nodes: None,
            reconcile_nodes: None,
        })
    }

    /// Sends the request to the endpoint and returns the response as a `ClientEndpointOutput`.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for the request.
    async fn run_stream_request(&self, client: &Client) -> Result<ClientEndpointOutput> {
        // Parse the check_path if it exists
        let check_path = self
            .check_path
            .clone()
            .and_then(|c| serde_json_path::JsonPath::parse(&c).ok());

        let reconcile_path = self
            .reconcile_path
            .clone()
            .and_then(|c| serde_json_path::JsonPath::parse(&c).ok());

        let start = Instant::now();
        let mut response = match self.method {
            SupportedMethod::Get => client
                .get(&self.url)
                .send()
                .await?
                .json_nl_stream::<Value>(usize::MAX),
            SupportedMethod::Post => client
                .post(&self.url)
                .body(self.body.clone().unwrap_or_default())
                .send()
                .await?
                .json_nl_stream::<Value>(usize::MAX),
        };

        let mut nodes = Vec::new();
        let mut reconcile_nodes = Vec::new();

        while let Some(data) = response.next().await {
            match data {
                Ok(body) => {
                    if let Some(path) = &check_path {
                        nodes.push(path.query(&body).exactly_one().unwrap_or_default().clone());
                    }

                    if let Some(reconcile_path) = &reconcile_path {
                        reconcile_nodes.push(
                            reconcile_path
                                .query(&body)
                                .exactly_one()
                                .unwrap_or_default()
                                .clone(),
                        );
                    }
                }
                Err(err) => match err.kind() {
                    // Ignore the error as it's due to the stream being closed due to the max length reached.
                    StreamBodyKind::MaxLenReachedError | StreamBodyKind::CodecError => {}
                    StreamBodyKind::InputOutputError => return Err(err.into()),
                },
            }
        }

        Ok(ClientEndpointOutput {
            elapsed: start.elapsed().as_millis(),
            status: StatusCode::OK,
            nodes: Some(nodes),
            reconcile_nodes: Some(reconcile_nodes),
        })
    }
}
