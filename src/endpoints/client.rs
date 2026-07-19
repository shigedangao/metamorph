use crate::client::CommonClient;
use crate::endpoints::params::{Endpoint, SupportedMethod};
use anyhow::{Result, anyhow};
use reqwest::StatusCode;
use serde_json::Value;
use std::{sync::Arc, time::Instant};

/// Represents a component of a client endpoint, including the URL, path, method, and body.
#[derive(Debug, Clone)]
pub struct ClientEndpointComponent {
    pub url: String,
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
    pub status: u16,
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
    /// * `stream_max_payload` - The maximum payload size for stream endpoints.
    pub async fn send(
        &self,
        client: Arc<dyn CommonClient>,
        stream_max_payload: usize,
    ) -> Result<ClientEndpointOutput> {
        match self.stream {
            true => self.run_stream_request(client, stream_max_payload).await,
            false => self.run_unary_request(client).await,
        }
    }

    /// Sends the request to the endpoint and returns the response as a `ClientEndpointOutput`.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for the request.
    /// * `max_payload_size` - The maximum payload size to use for the request.
    async fn run_unary_request(
        &self,
        client: Arc<dyn CommonClient>,
    ) -> Result<ClientEndpointOutput> {
        // Send the request and get the response
        let response = match self.method {
            SupportedMethod::Get => client.get(self.url.to_string()).await?,
            SupportedMethod::Post => {
                client
                    .post(self.url.to_string(), self.body.clone().unwrap_or_default())
                    .await?
            }
        };

        if let Some(check_path) = &self.check_path {
            let path = serde_json_path::JsonPath::parse(check_path)?;
            let Some(body) = response.body else {
                return Err(anyhow::anyhow!("No body returned from server"));
            };

            let node = path
                .query(&body)
                .exactly_one()
                .map_err(|e| anyhow!("Unable to found the desired path: {e}"))?;

            return Ok(ClientEndpointOutput {
                elapsed: response.duration.as_millis(),
                status: response.status,
                nodes: Some(vec![node.clone()]),
                reconcile_nodes: None,
            });
        }

        Ok(ClientEndpointOutput {
            elapsed: response.duration.as_millis(),
            status: response.status,
            nodes: None,
            reconcile_nodes: None,
        })
    }

    /// Sends the request to the endpoint and returns the response as a `ClientEndpointOutput`.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for the request.
    async fn run_stream_request(
        &self,
        client: Arc<dyn CommonClient>,
        stream_max_payload: usize,
    ) -> Result<ClientEndpointOutput> {
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
        let response = match self.method {
            SupportedMethod::Get => {
                client
                    .get_stream(self.url.to_string(), stream_max_payload)
                    .await?
            }
            SupportedMethod::Post => {
                client
                    .get_stream_with_body(
                        self.url.to_string(),
                        self.body.clone().unwrap_or_default(),
                        stream_max_payload,
                    )
                    .await?
            }
        };

        let mut nodes = Vec::new();
        let mut reconcile_nodes = Vec::new();

        for body in response {
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

        Ok(ClientEndpointOutput {
            elapsed: start.elapsed().as_millis(),
            status: StatusCode::OK.as_u16(),
            nodes: Some(nodes),
            reconcile_nodes: Some(reconcile_nodes),
        })
    }
}
