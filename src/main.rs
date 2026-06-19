use clap::Parser;
use cli::App;

mod cli;
mod endpoints;

#[tokio::main]
async fn main() {
    let app = App::parse();

    app.run().await.unwrap();
}
