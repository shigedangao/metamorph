use clap::Parser;
use cli::App;

mod cli;
mod endpoints;

#[tokio::main]
async fn main() {
    let app = App::parse();

    if let Err(err) = app.run().await {
        eprintln!("{err}");
    };
}
