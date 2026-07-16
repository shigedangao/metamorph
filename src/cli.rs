use crate::{
    client::Clients,
    endpoints::{EndpointRequestResult, Endpoints, values::Diff},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table};
use spinners::{Spinner, Spinners};
use std::collections::BTreeMap;
use tokio::{fs, task::JoinSet};

/// The main application struct.
#[derive(Parser, Debug)]
#[command(
    version = "0.1.6",
    about = "a CLI tool for benchmarking gRPC and HTTP endpoints"
)]
pub struct App {
    #[arg(short, long, help = "the path to the config file toml file")]
    config: String,

    #[arg(
        short,
        long,
        default_value = "15",
        help = "the read timeout for HTTP both unary & stream requests & gRPC unary requests (in seconds) (default: 15)"
    )]
    read_timeout: u64,

    #[arg(
        short,
        long,
        default_value = "4096",
        help = "the maximum payload size for streaming requests (in bytes) (default: 2048). For gRPC streaming it's recommended to at least set 4096 bytes"
    )]
    stream_max_payload: usize,

    #[arg(
        long,
        short = 'd',
        help = "the relative difference threshold in % for the benchmark results (default: 0)"
    )]
    relative_diff: Option<f64>,
}

impl App {
    /// Runs the application, reading the config file and making requests to the endpoints.
    pub async fn run(self) -> Result<()> {
        let bench = fs::read_to_string(&self.config)
            .await
            .map_err(|err| anyhow!("Unable to read the config file due to {err}"))?;

        // Parse the config file into an Endpoints struct
        let config = Endpoints::new(&bench)?;

        // Get endpoints and headers from the config
        // Build the http headers for origin & targets
        let (http_origin_headers, http_target_headers) = config.build_headers()?;
        // Build the grpc headers for origin & targets
        let (grpc_origin_headers, grpc_target_headers) = config.build_grpc_headers();

        // Build the endpoints from the config
        let endpoints =
            config.build_endpoints(&config.origin_base.method, &config.bench_base.method);

        let origin_client = Clients::new(
            config.origin_base.method,
            self.read_timeout,
            http_origin_headers,
            grpc_origin_headers,
            config.origin_base.url,
        )
        .await?;

        let target_client = Clients::new(
            config.bench_base.method,
            self.read_timeout,
            http_target_headers,
            grpc_target_headers,
            config.bench_base.url,
        )
        .await?;

        let mut set: JoinSet<Result<(String, EndpointRequestResult)>> = JoinSet::new();

        // Run through each endpoint and make a request to it
        for (name, endpoint) in endpoints {
            let o_client = origin_client.get_clients().clone();
            let t_client = target_client.get_clients().clone();

            set.spawn(async move {
                let mut sp = Spinner::new(Spinners::Dots, format!("Running {name} endpoints..."));

                let res = match endpoint
                    .run(
                        o_client,
                        t_client,
                        self.stream_max_payload,
                        self.relative_diff,
                    )
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        sp.stop_and_persist(
                            "🔴",
                            format!("Failed to process {name} endpoint due to: {e}"),
                        );

                        return Err(anyhow!(name));
                    }
                };
                sp.stop_and_persist("✅", format!("Finished processing {name} endpoints."));

                Ok((name, res))
            });
        }

        // Use a BTreeMap to sort results by endpoint name
        let mut results: BTreeMap<String, EndpointRequestResult> = BTreeMap::new();
        while let Some(res) = set.join_next().await {
            match res? {
                Ok((name, request_res)) => results.insert(name, request_res),
                Err(name) => {
                    // The error returns the name of the endpoint that has failed. We don't really care about the reason when displaying in the table.
                    results.insert(name.to_string(), EndpointRequestResult::default_error())
                }
            };
        }

        // Build the results
        let mut table = Table::new();
        table
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "endpoint name",
                "from",
                "target",
                "diff",
                "deltas (in ms)",
            ]);

        for (endpoint, res) in results {
            let diff = match &res.diff {
                Some(diffs) => {
                    let diff_str = diffs
                        .iter()
                        .map(|d| match d {
                            Diff::Output(s) => s.clone(),
                            Diff::UnableToCompare => "Unable to compare".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Cell::new(diff_str).fg(Color::DarkMagenta)
                }
                None => Cell::new("None").fg(Color::Green),
            };

            table.add_row(vec![
                Cell::new(endpoint).add_attribute(Attribute::Bold),
                Cell::new(res.from_status),
                Cell::new(res.target_status),
                diff,
                Cell::new(format!("{}", res.deltas)),
            ]);
        }

        println!("{table}");

        Ok(())
    }
}
