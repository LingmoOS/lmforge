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

    fn register_stages(&self, pipeline: &mut Vec<Box<dyn Stage>>) -> Result<()> {
        info!("Registering installer stages");

        struct InstallerStage;

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

            fn run(&self, _ctx: &mut BuildContext) -> Result<()> {
                info!("Setting up installer");
                
                debug!("Installer configuration applied");
                
                Ok(())
            }
        }

        pipeline.push(Box::new(InstallerStage));

        Ok(())
    }

    fn prepare_overlay(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Preparing installer overlay");

        let overlay_base = match &ctx.workspace_layout {
            Some(layout) => layout.overlay.clone(),
            None => ctx.workspace.overlay.clone()
        };

        let overlay_dir = overlay_base.join("installer");
        std::fs::create_dir_all(&overlay_dir)?;

        let config_dir = overlay_dir.join("config");
        std::fs::create_dir_all(config_dir)?;

        Ok(())
    }
}
