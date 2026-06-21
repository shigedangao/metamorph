use crate::endpoints::params::SupportedMethod;
use anyhow::Result;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use reqwest_streams::{JsonStreamResponse, error::StreamBodyKind};
use serde_json::Value;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ClientEndpointComponent {
    url: String,
    pub check_path: Option<String>,
    pub reconcile_path: Option<String>,
    method: SupportedMethod,
    stream: bool,
    body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientEndpointOutput {
    pub elapsed: u128,
    pub status: StatusCode,
    pub nodes: Option<Vec<Value>>,
    pub reconcile_nodes: Option<Vec<Value>>,
}

impl ClientEndpointComponent {
    pub fn new(
        url: String,
        check_path: Option<String>,
        reconcile_path: Option<String>,
        method: SupportedMethod,
        stream: bool,
        body: Option<String>,
    ) -> Self {
        Self {
            url,
            check_path,
            reconcile_path,
            method,
            stream,
            body,
        }
    }

    pub async fn send(&self, client: &Client) -> Result<ClientEndpointOutput> {
        match self.stream {
            true => self.run_stream_request(client).await,
            false => self.run_unary_request(client).await,
        }
    }

    async fn run_unary_request(&self, client: &Client) -> Result<ClientEndpointOutput> {
        let start = Instant::now();

        // Send the request and get the response
        let response = match self.method {
            SupportedMethod::Get => client.get(&self.url).send().await?,
            SupportedMethod::Post => {
                client
                    .post(&self.url)
                    .body(self.body.to_owned().unwrap_or_default())
                    .send()
                    .await?
            }
        };

        let elapsed = start.elapsed();
        // Get the status from the response
        let status = response.status();

        if let Some(check_path) = &self.check_path {
            let path = serde_json_path::JsonPath::parse(&check_path)?;
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
                .json_nl_stream::<Value>(2 * 1024),
            SupportedMethod::Post => client
                .post(&self.url)
                .body(self.body.to_owned().unwrap_or_default())
                .send()
                .await?
                .json_nl_stream::<Value>(2 * 1024),
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
                    _ => return Err(err.into()),
                },
            }
        }

        let elapsed = start.elapsed();
        Ok(ClientEndpointOutput {
            elapsed: elapsed.as_millis(),
            status: StatusCode::OK,
            nodes: Some(nodes),
            reconcile_nodes: Some(reconcile_nodes),
        })
    }
}
