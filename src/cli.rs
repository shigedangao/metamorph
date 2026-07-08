use crate::endpoints::{EndpointRequestResult, Endpoints, values::Diff};
use anyhow::{Result, anyhow};
use clap::Parser;
use comfy_table::Table;
use spinners::{Spinner, Spinners};
use std::{collections::BTreeMap, time::Duration};
use tokio::{fs, task::JoinSet};

/// The main application struct.
#[derive(Parser, Debug)]
pub struct App {
    #[arg(short, long)]
    config: String,

    #[arg(short, long, default_value = "15")]
    read_timeout: u64,

    #[arg(short, long, default_value = "2048")]
    stream_max_payload: usize,

    #[arg(short, long)]
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
        let (origin_headers, target_headers) = config.build_headers()?;
        let endpoints = config.build_endpoints();

        let origin_client = reqwest::ClientBuilder::new()
            .default_headers(origin_headers)
            .timeout(Duration::from_secs(self.read_timeout))
            .build()?;

        let target_client = reqwest::ClientBuilder::new()
            .default_headers(target_headers)
            .timeout(Duration::from_secs(self.read_timeout))
            .build()?;

        let mut set: JoinSet<Result<(String, EndpointRequestResult)>> = JoinSet::new();

        // Run through each endpoint and make a request to it
        for (name, endpoint) in endpoints {
            let o_client = origin_client.clone();
            let t_client = target_client.clone();

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
                            "✖",
                            format!("Failed to process {name} endpoint due to: {e}"),
                        );

                        return Err(anyhow!(name));
                    }
                };
                sp.stop_and_persist("✔", format!("Finished processing {name} endpoints."));

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
        table.set_header(vec![
            "endpoint name",
            "from",
            "target",
            "diff",
            "deltas (in ms)",
        ]);

        for (endpoint, res) in results {
            let diff = match &res.diff {
                Some(diffs) => diffs
                    .iter()
                    .map(|d| match d {
                        Diff::Output(s) => s.clone(),
                        Diff::UnableToCompare => "Unable to compare".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => "None".to_string(),
            };

            table.add_row(vec![
                endpoint,
                res.from_status,
                res.target_status,
                diff,
                format!("{}", res.deltas),
            ]);
        }

        println!("{table}");

        Ok(())
    }
}
