use clap::Parser;
use anyhow::Result;
use tracing::info;

use super::cli::Cli;

#[derive(Debug, Parser)]
pub struct BuildCommand {
    /// Target to build (iso, live, rootfs)
    pub target: String,

    /// Preset or profile name
    pub profile: Option<String>,

    #[arg(long)]
    pub desktop: bool,

    #[arg(long)]
    pub live: bool,

    #[arg(long)]
    pub installer: bool,

    #[arg(long)]
    pub secureboot: bool,

    #[arg(long)]
    pub clean: bool,

    #[arg(long)]
    pub dry_run: bool,
}

impl BuildCommand {
    pub fn execute(&self, cli: &Cli) -> Result<()> {
        info!(
            "Building target '{}' with profile '{:?}'",
            self.target,
            self.profile
        );

        let mut features = Vec::new();
        
        if self.desktop {
            features.push("desktop".to_string());
        }
        if self.live {
            features.push("live".to_string());
        }
        if self.installer {
            features.push("installer".to_string());
        }

        info!("Enabled features: {:?}", features);

        if self.dry_run {
            println!("Dry run mode - would build {} with features {:?}", self.target, features);
            return Ok(());
        }

        use crate::engine::orchestrator::BuildOrchestrator;
        
        let orchestrator = BuildOrchestrator::new()
            .with_target(&self.target)
            .with_profile(self.profile.as_deref())
            .with_features(features)
            .with_clean(self.clean)
            .with_log_level(cli.get_log_level());

        orchestrator.run(cli)?;

        Ok(())
    }
}
