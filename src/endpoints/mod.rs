use crate::endpoints::params::{BenchEndpointComponent, CheckPath};
use anyhow::Result;
use futures::{StreamExt, stream::BoxStream};
use reqwest::{
    Response, StatusCode,
    header::{HeaderMap, HeaderName},
};
use reqwest_streams::{JsonStreamResponse as _, error::StreamBodyError};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use tokio::task::JoinSet;
use toml::Value;

mod params;

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
#[derive(Debug, Clone, Default)]
pub struct BuildEndpoint {
    from: String,
    target: String,
    from_check_path: Option<CheckPath>,
    target_check_path: Option<CheckPath>,
    from_body_params: Option<String>,
    target_body_params: Option<String>,
    stream: bool,
}

/// InnerEndpointRequestResult represents the result of a request to an endpoint.
#[derive(Debug)]
pub enum InnerEndpointRequestResult {
    From(StatusCode, u128, Option<JsonValue>),
    Target(StatusCode, u128, Option<JsonValue>),
}

/// EndpointRequestResult represents the result of a request to an endpoint.
#[derive(Default)]
pub struct EndpointRequestResult {
    pub from: String,
    pub target: String,
    pub deltas: u128,
    pub diff: Option<Diff>,
}

/// Diff represents the difference between two endpoints.
pub enum Diff {
    String(String),
    Number(f64),
}

pub enum ReqwestPayload {
    Body(Vec<JsonValue>, StatusCode),
    Stream(Vec<JsonValue>, StatusCode),
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

            let mut build_endpoint = BuildEndpoint {
                from: format!("{}/{}", self.origin_base_url, from),
                target: format!("{}/{}", self.bench_base_url, target),
                from_check_path: parsed.from.check_path.clone(),
                target_check_path: parsed.target.check_path.clone(),
                ..Default::default()
            };

            if self.stream {
                build_endpoint.from_body_params = parsed.from.params.get("args").map(|v| v.clone());
                build_endpoint.target_body_params =
                    parsed.target.params.get("args").map(|v| v.clone());
                build_endpoint.stream = true;
            }

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
                    HeaderName::from_lowercase(config.name.as_bytes())?,
                    config.value.parse().unwrap(),
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
            let start = Instant::now();
            let _ = from_client.get(&self.from).send().await?;
            let duration = start.elapsed();

            let (res, elapsed) = match self.stream {
                false => {
                    let res = from_client.get(&self.from).send().await?;
                    let elapsed = start.elapsed();
                    let status = res.status();

                    let body = res.json::<JsonValue>().await?;

                    (ReqwestPayload::Body(vec![body], status), elapsed)
                }
                true => {
                    let stream = from_client
                        .get(&self.from)
                        .send()
                        .await?
                        .json_array_stream::<JsonValue>(128)
                        .boxed();

                    let (res, status, elapsed) = process_stream_body(stream, start).await;
                    (ReqwestPayload::Stream(res, status), elapsed)
                }
            };

            // Get the status code of the response
            let (body, status) = match res {
                ReqwestPayload::Body(body, status) => (body, status),
                ReqwestPayload::Stream(body, status) => (body, status),
            };

            // Compile the jmespath expression if check_path is present
            if let Some(check_path) = &self.from_check_path {
                match parse_reqwest_body(body, check_path).await {
                    Ok(node) => {
                        return Ok(InnerEndpointRequestResult::From(
                            status,
                            duration.as_millis(),
                            Some(node),
                        ));
                    }
                    Err(err) => {
                        println!("Unable to parse the body due to: {err}")
                    }
                }
            }

            Ok(InnerEndpointRequestResult::From(
                status,
                duration.as_millis(),
                None,
            ))
        });

        let target_client = client.clone();
        set.spawn(async move {
            let start = Instant::now();
            let _ = target_client.get(&self.target).send().await?;
            let duration = start.elapsed();

            let (res, elapsed) = match self.stream {
                false => {
                    let res = target_client.get(&self.target).send().await?;
                    let res = res.status();

                    let elapsed = start.elapsed();

                    let body = res.json::<JsonValue>().await?;

                    (ReqwestPayload::Body(vec![body], status), elapsed)
                }
                true => {
                    let stream = target_client
                        .get(&self.target)
                        .send()
                        .await?
                        .json_array_stream::<JsonValue>(128)
                        .boxed();

                    let (res, status, elapsed) = process_stream_body(stream, start).await;
                    (ReqwestPayload::Stream(res, status), elapsed)
                }
            };

            // Get the status code of the response
            let (body, status) = match res {
                ReqwestPayload::Body(body, status) => (body, status),
                ReqwestPayload::Stream(body, status) => (body, status),
            };

            // Compile the jmespath expression if check_path is present
            if let Some(check_path) = self.target_check_path {
                match parse_reqwest_body(body, &check_path).await {
                    Ok(node) => {
                        return Ok(InnerEndpointRequestResult::Target(
                            status,
                            duration.as_millis(),
                            Some(node),
                        ));
                    }
                    Err(err) => {
                        println!("Unable to parse the body due to: {err}")
                    }
                }
            }

            Ok(InnerEndpointRequestResult::Target(
                status,
                duration.as_millis(),
                None,
            ))
        });

        // store durations
        let mut from_duration = 0;
        let mut target_duration = 0;

        // store node value
        let mut from_node: Option<JsonValue> = None;
        let mut target_node: Option<JsonValue> = None;

        let mut endpoint_result = EndpointRequestResult::default();

        while let Some(res) = set.join_next().await {
            match res?? {
                InnerEndpointRequestResult::From(status, dur, node) => {
                    endpoint_result.from = status.to_string();
                    from_duration = dur;
                    from_node = node;
                }
                InnerEndpointRequestResult::Target(status, dur, node) => {
                    endpoint_result.target = status.to_string();
                    target_duration = dur;
                    target_node = node;
                }
            }
        }

        // Calculating the duration difference between the two requests
        endpoint_result.deltas = target_duration.saturating_sub(from_duration);

        // Checking the diff between the two nodes and storing it in the result
        // The diff comes from the check_path provided by the endpoint configuration
        let diff = from_node.zip(target_node).and_then(|(f, t)| match (f, t) {
            (JsonValue::String(f), JsonValue::String(t)) => {
                if f != t {
                    Some(Diff::String(t.to_string()))
                } else {
                    None
                }
            }
            (JsonValue::Number(f), JsonValue::Number(t)) => Some(Diff::Number(
                t.as_f64().unwrap_or_default() - f.as_f64().unwrap_or_default(),
            )),
            _ => None,
        });

        endpoint_result.diff = diff;

        Ok(endpoint_result)
    }
}

/// Parses the response body using the provided check path and returns the matched node.
///
/// # Arguments
///
/// * `resp` - The response from the HTTP request.
/// * `check_path` - The check path to use for parsing the response body.
///
/// # Returns
///
/// A `Result` containing the matched node, or an error if parsing fails.
async fn parse_reqwest_body(resp: Vec<JsonValue>, check_path: &CheckPath) -> Result<JsonValue> {
    let path = serde_json_path::JsonPath::parse(&check_path.path)?;

    let node = path.query(&body).exactly_one().unwrap_or_default();

    Ok(node.clone())
}

async fn process_stream_body<'a>(
    mut stream: BoxStream<'a, Result<JsonValue, StreamBodyError>>,
    start: Instant,
) -> (Vec<JsonValue>, StatusCode, Duration) {
    let mut res = Vec::new();

    while let Some(data) = stream.next().await {
        match data {
            Ok(d) => res.push(d),
            Err(_) => return (res, StatusCode::BAD_REQUEST, start.elapsed()),
        }
    }

    (res, StatusCode::OK, start.elapsed())
}
