use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod command;
mod engine;
mod runtime;
mod domain;
mod platform;
mod stages;
mod features;
mod infra;
mod config;
mod telemetry;

use command::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    let cli = Cli::parse();
    
    tracing::info!("lmforge v{} starting", env!("CARGO_PKG_VERSION"));

    cli.execute().await?;

    Ok(())
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .init();
}
