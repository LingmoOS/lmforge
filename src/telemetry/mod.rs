pub mod build_id;
pub mod console;
pub mod file_logger;
pub mod runtime;
pub mod context;
pub mod layer;

use console::ConsoleLayer;
use file_logger::FileLogLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init(build_id: &str, log_dir: &std::path::Path) -> anyhow::Result<()> {
    let console_layer = ConsoleLayer::new();
    let file_layer = FileLogLayer::new(build_id, log_dir)?;

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    Ok(())
}
