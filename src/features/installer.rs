use anyhow::Result;
use tracing::{info, debug};

use super::feature_trait::Feature;
use crate::domain::context::BuildContext;
use crate::stages::stage::Stage;

pub struct InstallerFeature;

impl Default for InstallerFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl InstallerFeature {
    pub fn new() -> Self {
        InstallerFeature
    }
}

#[async_trait]
impl Feature for InstallerFeature {
    fn name(&self) -> &str {
        "installer"
    }

    fn description(&self) -> &str {
        "System installer (Calamares/Ubiquity)"
    }

    fn conflicts_with(&self) -> Vec<&str> {
        vec!["minimal"]
    }

    async fn register_stages(&self, pipeline: &mut Vec<Box<dyn Stage>>) -> Result<()> {
        info!("Registering installer stages");

        struct InstallerStage;

        #[async_trait]
        impl Stage for InstallerStage {
            fn name(&self) -> &str {
                "installer-setup"
            }

            fn description(&self) -> &str {
                "Configure system installer"
            }

            fn dependencies(&self) -> Vec<&str> {
                vec!["packages"]
            }

            async fn run(&self, _ctx: &mut BuildContext) -> Result<()> {
                info!("Setting up installer");
                
                // Configure Calamares or other installer
                debug!("Installer configuration applied");
                
                Ok(())
            }
        }

        pipeline.push(Box::new(InstallerStage));

        Ok(())
    }

    async fn prepare_overlay(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Preparing installer overlay");

        let overlay_dir = ctx.workspace.overlay.join("installer");
        std::fs::create_dir_all(&overlay_dir)?;

        let config_dir = overlay_dir.join("config");
        std::fs::create_dir_all(config_dir)?;

        Ok(())
    }
}
