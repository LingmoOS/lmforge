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
mod telemetry;

use command::cli::Cli;
use telemetry::build_id::BuildId as BuildIdStruct;

fn main() -> anyhow::Result<()> {
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

    cli.execute()?;

    Ok(())
}
