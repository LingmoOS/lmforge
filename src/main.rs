use clap::Parser;
use std::path::PathBuf;
use tracing::info;

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
use telemetry::{BuildId, build_id::BuildId as BuildIdStruct};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    let build_id = BuildIdStruct::new();
    let output_dir = cli.output.clone().unwrap_or_else(|| PathBuf::from("./output"));
    
    let log_dir = build_id.logs_dir(&output_dir);
    telemetry::init(&build_id.id, &log_dir)?;
    
    info!(
        build_id = %build_id,
        version = env!("CARGO_PKG_VERSION"),
        "lmforge starting"
    );

    cli.execute().await?;

    Ok(())
}
