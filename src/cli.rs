use crate::endpoints::{EndpointRequestResult, Endpoints};
use anyhow::Result;
use clap::Parser;
use comfy_table::Table;
use spinners::{Spinner, Spinners};
use std::{collections::HashMap, time::Duration};
use tokio::{fs, task::JoinSet};

#[derive(Parser, Debug)]
pub struct App {
    #[arg(short, long)]
    config: String,

    #[arg(short, long)]
    token: Option<String>,
}

impl App {
    pub async fn run(&self) -> Result<()> {
        let bench = fs::read_to_string(&self.config).await?;
        let config = Endpoints::new(bench)?;

        // Get endpoints and headers from the config
        let endpoints = config.build_endpoints();
        let headers = config.build_headers()?;

        let client = reqwest::ClientBuilder::new()
            .default_headers(headers)
            .timeout(Duration::from_secs(5))
            .build()?;

        let mut set: JoinSet<Result<(String, EndpointRequestResult)>> = JoinSet::new();

        // Run through each endpoint and make a request to it
        for (name, endpoint) in endpoints {
            let client = client.clone();
            set.spawn(async move {
                let mut sp = Spinner::new(Spinners::Dots, format!("Running {name} endpoints..."));

                let res = endpoint.run(client).await?;
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
        table.set_header(vec!["endpoint name", "from", "target", "deltas (in ms)"]);

        results.iter().for_each(|(endpoint, res)| {
            table.add_row(vec![
                endpoint.clone(),
                res.from.clone(),
                res.target.clone(),
                format!("{}", res.deltas),
            ]);
        });

        println!("{table}");

        Ok(())
    }
}
