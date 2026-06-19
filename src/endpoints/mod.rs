use crate::endpoints::params::BenchEndpointComponent;
use anyhow::Result;
use reqwest::{
    StatusCode,
    header::{HeaderMap, HeaderName},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;
use tokio::task::JoinSet;
use toml::Value;

mod params;

#[derive(Debug, Deserialize)]
struct HeaderConfig {
    value: String,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct Endpoints {
    origin_base_url: String,
    bench_base_url: String,
    headers: Option<HashMap<String, HeaderConfig>>,

    #[serde(flatten)]
    _endpoints: HashMap<String, Value>,
    #[serde(skip)]
    _parsed_endpoints: HashMap<String, BenchEndpointComponent>,
}

#[derive(Debug, Clone)]
pub struct BuildEndpoint {
    from: String,
    target: String,
}

#[derive(Debug)]
pub enum InnerEndpointRequestResult {
    From(StatusCode, u128),
    Target(StatusCode, u128),
}

#[derive(Default)]
pub struct EndpointRequestResult {
    pub from: String,
    pub target: String,
    pub deltas: u128,
}

impl Endpoints {
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

    pub fn build_endpoints(&self) -> HashMap<String, BuildEndpoint> {
        let mut endpoints = HashMap::new();

        for (name, parsed) in &self._parsed_endpoints {
            let (from, target) = parsed.template();

            let build_endpoint = BuildEndpoint {
                from: format!("{}/{}", self.origin_base_url, from),
                target: format!("{}/{}", self.bench_base_url, target),
            };

            endpoints.insert(name.clone(), build_endpoint);
        }

        endpoints
    }

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
    pub async fn run(self, client: reqwest::Client) -> Result<EndpointRequestResult> {
        let mut set: JoinSet<Result<InnerEndpointRequestResult>> = JoinSet::new();

        let from_client = client.clone();
        set.spawn(async move {
            let start = Instant::now();
            let res = from_client.get(self.from.clone()).send().await?;
            let duration = start.elapsed();

            Ok(InnerEndpointRequestResult::From(
                res.status(),
                duration.as_millis(),
            ))
        });

        let target_client = client.clone();
        set.spawn(async move {
            let start = Instant::now();
            let res = target_client.get(self.target.clone()).send().await?;
            let duration = start.elapsed();

            Ok(InnerEndpointRequestResult::Target(
                res.status(),
                duration.as_millis(),
            ))
        });

        let mut from_duration = 0;
        let mut target_duration = 0;
        let mut endpoint_result = EndpointRequestResult::default();

        while let Some(res) = set.join_next().await {
            match res?? {
                InnerEndpointRequestResult::From(status, dur) => {
                    endpoint_result.from = status.to_string();
                    from_duration = dur;
                }
                InnerEndpointRequestResult::Target(status, dur) => {
                    endpoint_result.target = status.to_string();
                    target_duration = dur;
                }
            }
        }

        endpoint_result.deltas = target_duration.saturating_sub(from_duration);

        Ok(endpoint_result)
    }
}
