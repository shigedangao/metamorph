use crate::{
    client::{ClientError, CommonClient, TransportMethod},
    endpoints::{
        params::BenchEndpointComponent,
        values::{Diff, ValueComparison},
    },
};
use client::{ClientEndpointComponent, ClientEndpointOutput};
use reqwest::header::{HeaderMap, HeaderName};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tokio::task::JoinSet;
use toml::Value;

mod client;
mod params;
pub mod values;

/// Represents a header configuration for an endpoint.
#[derive(Debug, Deserialize, Clone)]
struct HeaderConfig {
    value: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct HeadersParams {
    origin: HashMap<String, HeaderConfig>,
    bench: HashMap<String, HeaderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct BaseParams {
    pub url: String,
    #[serde(default)]
    pub method: TransportMethod,
}

/// Represents a parsed endpoint component.
#[derive(Debug, Deserialize)]
pub struct Endpoints {
    pub origin_base: BaseParams,
    pub bench_base: BaseParams,
    headers: Option<HeadersParams>,
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
    pub from_status: String,
    pub target_status: String,
    pub deltas: u128,
    pub diff: Option<Vec<values::Diff>>,
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
    pub fn new(config: &str) -> anyhow::Result<Self> {
        let mut endpoints: Endpoints = toml::from_str(config)?;

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
    pub fn build_endpoints(
        &self,
        from_transport_method: &TransportMethod,
        target_transport_method: &TransportMethod,
    ) -> HashMap<String, BuildEndpoint> {
        let mut endpoints = HashMap::new();

        for (name, parsed) in &self._parsed_endpoints {
            let (from, target) = parsed.template();
            let (from_body, target_body) = parsed.get_body();

            let from_endpoint = match from_transport_method {
                TransportMethod::Http => format!("{}/{}", self.origin_base.url, from),
                _ => from,
            };

            let target_endpoint = match target_transport_method {
                TransportMethod::Http => format!("{}/{}", self.bench_base.url, target),
                _ => target,
            };

            let build_endpoint = BuildEndpoint {
                from: ClientEndpointComponent::new(
                    from_endpoint,
                    parsed.from.clone(),
                    self.stream,
                    from_body,
                ),
                target: ClientEndpointComponent::new(
                    target_endpoint,
                    parsed.target.clone(),
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
    pub fn build_headers(&self) -> anyhow::Result<(HeaderMap, HeaderMap)> {
        let mut origin_headers = HeaderMap::new();
        let mut target_headers = HeaderMap::new();

        if let Some(header_config) = &self.headers {
            for o_config in header_config.origin.values() {
                origin_headers.insert(
                    HeaderName::from_bytes(o_config.name.as_bytes())?,
                    o_config.value.parse()?,
                );
            }

            for t_config in header_config.bench.values() {
                target_headers.insert(
                    HeaderName::from_bytes(t_config.name.as_bytes())?,
                    t_config.value.parse()?,
                );
            }
        }

        Ok((origin_headers, target_headers))
    }

    /// Returns the headers as a vector of `(name, value)` pairs for GRPC.
    ///
    /// The headers are extracted from the `headers` field of the `Endpoints` struct.
    #[allow(clippy::type_complexity)]
    pub fn build_grpc_headers(&self) -> (Vec<(String, String)>, Vec<(String, String)>) {
        match self.headers.as_ref() {
            Some(headers) => {
                // We can safely unwrap here as we checked for None above.
                let origin_headers = headers
                    .origin
                    .clone()
                    .into_values()
                    .map(|v| (v.name, v.value))
                    .collect::<Vec<_>>();

                let bench_headers = headers
                    .bench
                    .clone()
                    .into_values()
                    .map(|v| (v.name, v.value))
                    .collect::<Vec<_>>();

                (origin_headers, bench_headers)
            }
            None => (Vec::new(), Vec::new()),
        }
    }
}

impl BuildEndpoint {
    /// new parses the given endpoint name and value into a `BuildEndpoint` struct.
    ///
    /// # Arguments
    ///
    /// * `value` - The value of the endpoint, as a `Value` from the TOML parser.
    /// * `max_payload_size` - The maximum payload size for the endpoint.
    ///
    /// # Returns
    ///
    /// A `Result` containing the parsed `BuildEndpoint` struct, or an error if parsing fails.
    pub async fn run(
        self,
        o_client: Arc<dyn CommonClient>,
        t_client: Arc<dyn CommonClient>,
        stream_max_payload: usize,
        relative_diff: Option<f64>,
    ) -> Result<EndpointRequestResult, ClientError> {
        let mut set: JoinSet<Result<InnerEndpointRequestResult, ClientError>> = JoinSet::new();

        let from_client = o_client.clone();
        set.spawn(async move {
            let res = self.from.send(from_client, stream_max_payload).await?;

            Ok(InnerEndpointRequestResult::From(res))
        });

        let target_client = t_client.clone();
        set.spawn(async move {
            let res = self.target.send(target_client, stream_max_payload).await?;

            Ok(InnerEndpointRequestResult::Target(res))
        });

        let mut from_client_output = ClientEndpointOutput::default();
        let mut target_client_output = ClientEndpointOutput::default();

        while let Some(res) = set.join_next().await {
            if let Ok(payload) = res {
                match payload? {
                    InnerEndpointRequestResult::From(output) => {
                        from_client_output = output;
                    }
                    InnerEndpointRequestResult::Target(output) => {
                        target_client_output = output;
                    }
                }
            }
        }

        // Calculating the duration difference between the two requests
        let mut endpoint_result = EndpointRequestResult {
            deltas: target_client_output
                .elapsed
                .saturating_sub(from_client_output.elapsed),
            from_status: from_client_output.status.to_string(),
            target_status: target_client_output.status.to_string(),
            ..Default::default()
        };

        // Compare the diff between two vec of node values whenever provided
        if let Some((f_nodes, t_nodes)) = from_client_output.nodes.zip(target_client_output.nodes) {
            if f_nodes.is_empty() || t_nodes.is_empty() {
                return Err(ClientError::with_reason(format!(
                    "Data could not be fetch from nodes: from datasets length: {}, target datasets length: {}",
                    f_nodes.len(),
                    t_nodes.len()
                )));
            }

            let comparison_handle = ValueComparison::new(
                &f_nodes,
                &t_nodes,
                from_client_output.reconcile_nodes,
                target_client_output.reconcile_nodes,
            );

            endpoint_result.diff =
                comparison_handle.compare_values(relative_diff.unwrap_or_default());
        }

        Ok(endpoint_result)
    }
}

impl EndpointRequestResult {
    /// Returns a default error result with ❌ status and an empty diff.
    pub fn with_default_error(reason: String) -> Self {
        Self {
            from_status: "❌".to_string(),
            target_status: "❌".to_string(),
            deltas: 0,
            diff: Some(vec![Diff::UnableToCompare(reason)]),
        }
    }
}
