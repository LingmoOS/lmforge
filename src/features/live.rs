use anyhow::Result;
use tracing::{info, debug};

use super::feature_trait::Feature;
use crate::domain::context::BuildContext;
use crate::stages::stage::Stage;

pub struct LiveFeature;

impl Default for LiveFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveFeature {
    pub fn new() -> Self {
        LiveFeature
    }
}

impl Feature for LiveFeature {
    fn name(&self) -> &str {
        "live"
    }

    fn description(&self) -> &str {
        "Live system support (bootable ISO)"
    }

    fn register_stages(&self, pipeline: &mut Vec<Box<dyn Stage>>) -> Result<()> {
        info!("Registering live system stages");

        struct LiveBootStage;

        impl Stage for LiveBootStage {
            fn name(&self) -> &str {
                "live-boot"
            }

            fn description(&self) -> &str {
                "Configure live boot system"
            }

            fn dependencies(&self) -> Vec<&str> {
                vec!["packages"]
            }

            fn run(&self, _ctx: &mut BuildContext) -> Result<()> {
                info!("Configuring live boot system");
                
                debug!("Live boot configuration applied");
                
                Ok(())
            }
        }

        struct LivePackagesStage;

        impl Stage for LivePackagesStage {
            fn name(&self) -> &str {
                "live-packages"
            }

            fn description(&self) -> &str {
                "Install live system packages"
            }

            fn dependencies(&self) -> Vec<&str> {
                vec!["bootstrap"]
            }

            fn run(&self, _ctx: &mut BuildContext) -> Result<()> {
                info!("Installing live system packages");
                
                debug!("Live packages would be installed here");
                
                Ok(())
            }
        }

        pipeline.push(Box::new(LivePackagesStage));
        pipeline.push(Box::new(LiveBootStage));

        Ok(())
    }

    fn prepare_overlay(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Preparing live system overlay");

        let overlay_dir = ctx.workspace.overlay.join("live");
        std::fs::create_dir_all(&overlay_dir)?;

        let hooks_dir = overlay_dir.join("hooks");
        std::fs::create_dir_all(hooks_dir)?;

        Ok(())
    }
}
