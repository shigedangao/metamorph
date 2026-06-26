use crate::endpoints::{EndpointRequestResult, Endpoints, values::Diff};
use anyhow::Result;
use clap::Parser;
use comfy_table::Table;
use spinners::{Spinner, Spinners};
use std::{collections::HashMap, time::Duration};
use tokio::{fs, task::JoinSet};

/// The main application struct.
#[derive(Parser, Debug)]
pub struct App {
    #[arg(short, long)]
    config: String,

    #[arg(short, long, default_value = "15")]
    read_timeout: u64,
}

impl App {
    /// Runs the application, reading the config file and making requests to the endpoints.
    pub async fn run(&self) -> Result<()> {
        let bench = fs::read_to_string(&self.config).await?;
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

                let res = match endpoint.run(o_client, t_client).await {
                    Ok(res) => res,
                    Err(e) => {
                        sp.stop_and_persist(
                            "✖",
                            format!("Failed to process {name} endpoints: {e}"),
                        );

                        return Ok((name, EndpointRequestResult::default()));
                    }
                };
                sp.stop_and_persist("✔", format!("Finished processing {name} endpoints."));

                Ok((name, res))
            });
        }

        let mut results: HashMap<String, EndpointRequestResult> = HashMap::new();
        while let Some(res) = set.join_next().await {
            let (name, request_res) = res??;

            results.insert(name, request_res);
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

        results.into_iter().for_each(|(endpoint, res)| {
            let diff = match &res.diff {
                Some(diffs) => diffs
                    .iter()
                    .map(|d| match d {
                        Diff::Result(s) => s.clone(),
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
        });

        println!("{table}");

        Ok(())
    }
}
