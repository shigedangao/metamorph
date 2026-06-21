use crate::endpoints::{params::BenchEndpointComponent, values::ValueComparison};
use anyhow::Result;
use client::{ClientEndpointComponent, ClientEndpointOutput};
use reqwest::header::{HeaderMap, HeaderName};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::task::JoinSet;
use toml::Value;

mod client;
mod params;
pub mod values;

/// Represents a header configuration for an endpoint.
#[derive(Debug, Deserialize)]
struct HeaderConfig {
    value: String,
    name: String,
}

/// Represents a parsed endpoint component.
#[derive(Debug, Deserialize)]
pub struct Endpoints {
    origin_base_url: String,
    bench_base_url: String,
    headers: Option<HashMap<String, HeaderConfig>>,
    #[serde(default)]
    stream: bool,

    #[serde(flatten)]
    _endpoints: HashMap<String, Value>,
    #[serde(skip)]
    _parsed_endpoints: HashMap<String, BenchEndpointComponent>,
}

/// BuildEndpoint represents a parsed endpoint component.
#[derive(Debug, Clone)]
pub struct BuildEndpoint {
    from: ClientEndpointComponent,
    target: ClientEndpointComponent,
}

/// InnerEndpointRequestResult represents the result of a request to an endpoint.
#[derive(Debug)]
pub enum InnerEndpointRequestResult {
    From(ClientEndpointOutput),
    Target(ClientEndpointOutput),
}

/// EndpointRequestResult represents the result of a request to an endpoint.
#[derive(Default, Debug)]
pub struct EndpointRequestResult {
    pub from: String,
    pub target: String,
    pub deltas: u128,
    pub diff: Option<values::Diff>,
}

impl Endpoints {
    /// new parses the given TOML config string into an Endpoints struct.
    ///
    /// # Arguments
    ///
    /// * `config` - A TOML string representing the endpoints configuration.
    ///
    /// # Returns
    ///
    /// A `Result` containing the parsed `Endpoints` struct, or an error if parsing fails.
    pub fn new(config: String) -> Result<Self> {
        let mut endpoints: Endpoints = toml::from_str(&config)?;

        // Loop through the table of the endpoints and parse each endpoint into a BenchEndpointComponent
        for (name, value) in &endpoints._endpoints {
            let parsed: BenchEndpointComponent =
                BenchEndpointComponent::deserialize(value.clone())?;
            endpoints._parsed_endpoints.insert(name.clone(), parsed);
        }

        Ok(endpoints)
    }

    /// build_endpoints builds a HashMap of `BuildEndpoint` structs from the parsed endpoints.
    ///
    /// # Returns
    ///
    /// A `HashMap` where the key is the endpoint name and the value is the `BuildEndpoint` struct.
    pub fn build_endpoints(&self) -> HashMap<String, BuildEndpoint> {
        let mut endpoints = HashMap::new();

        for (name, parsed) in &self._parsed_endpoints {
            let (from, target) = parsed.template();
            let (from_body, target_body) = parsed.get_body();

            let build_endpoint = BuildEndpoint {
                from: ClientEndpointComponent::new(
                    format!("{}/{}", self.origin_base_url, from),
                    parsed.from.check_path.clone(),
                    parsed.from.reconcile_path.clone(),
                    parsed.from.method.clone(),
                    self.stream,
                    from_body,
                ),
                target: ClientEndpointComponent::new(
                    format!("{}/{}", self.bench_base_url, target),
                    parsed.target.check_path.clone(),
                    parsed.target.reconcile_path.clone(),
                    parsed.target.method.clone(),
                    self.stream,
                    target_body,
                ),
            };

            endpoints.insert(name.clone(), build_endpoint);
        }

        endpoints
    }

    /// build_headers builds a `HeaderMap` from the parsed headers configuration.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `HeaderMap`, or an error if building fails.
    pub fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        if let Some(header_config) = &self.headers {
            for (_, config) in header_config {
                headers.insert(
                    HeaderName::from_bytes(config.name.as_bytes())?,
                    config.value.parse()?,
                );
            }
        }

        Ok(headers)
    }
}

impl BuildEndpoint {
    /// new parses the given endpoint name and value into a `BuildEndpoint` struct.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the endpoint.
    /// * `value` - The value of the endpoint, as a `Value` from the TOML parser.
    ///
    /// # Returns
    ///
    /// A `Result` containing the parsed `BuildEndpoint` struct, or an error if parsing fails.
    pub async fn run(self, client: reqwest::Client) -> Result<EndpointRequestResult> {
        let mut set: JoinSet<Result<InnerEndpointRequestResult>> = JoinSet::new();

        let from_client = client.clone();
        set.spawn(async move {
            let res = self.from.clone().send(&from_client).await?;

            Ok(InnerEndpointRequestResult::From(res))
        });

        let target_client = client.clone();
        set.spawn(async move {
            let res = self.target.clone().send(&target_client).await?;

            Ok(InnerEndpointRequestResult::Target(res))
        });

        // store durations
        let mut from_duration = 0;
        let mut target_duration = 0;

        // store node value
        let mut from_nodes: Option<Vec<JsonValue>> = None;
        let mut target_nodes: Option<Vec<JsonValue>> = None;

        // store reconcile nodes
        let mut from_reconcile_nodes: Option<Vec<JsonValue>> = None;
        let mut target_reconcile_nodes: Option<Vec<JsonValue>> = None;

        let mut endpoint_result = EndpointRequestResult::default();

        while let Some(res) = set.join_next().await {
            match res?? {
                InnerEndpointRequestResult::From(output) => {
                    endpoint_result.from = output.status.to_string();
                    from_duration = output.elapsed;
                    from_nodes = output.nodes;
                    from_reconcile_nodes = output.reconcile_nodes;
                }
                InnerEndpointRequestResult::Target(output) => {
                    endpoint_result.target = output.status.to_string();
                    target_duration = output.elapsed;
                    target_nodes = output.nodes;
                    target_reconcile_nodes = output.reconcile_nodes;
                }
            }
        }

        // Calculating the duration difference between the two requests
        endpoint_result.deltas = target_duration.saturating_sub(from_duration);

        // Compare the diff between two vec of node values whenever provided
        if let Some((f_nodes, t_nodes)) = from_nodes.zip(target_nodes) {
            let comparison_handle = ValueComparison::new(
                f_nodes,
                t_nodes,
                from_reconcile_nodes,
                target_reconcile_nodes,
            );

            endpoint_result.diff = comparison_handle.compare_values();
        }

        Ok(endpoint_result)
    }
}
