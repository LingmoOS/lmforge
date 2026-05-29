use anyhow::Result;
use tracing::{info, debug};

use super::feature_trait::Feature;
use crate::domain::context::BuildContext;
use crate::stages::stage::Stage;

pub struct DesktopFeature {
    desktop_environment: String,
}

impl DesktopFeature {
    pub fn new(desktop: impl Into<String>) -> Self {
        DesktopFeature {
            desktop_environment: desktop.into(),
        }
    }

    pub fn gnome() -> Self {
        Self::new("gnome")
    }

    pub fn kde() -> Self {
        Self::new("kde")
    }

    pub fn xfce() -> Self {
        Self::new("xfce")
    }

    fn get_packages(&self) -> Vec<&'static str> {
        match self.desktop_environment.as_str() {
            "gnome" => vec![
                "gnome-core",
                "gnome-shell",
                "gnome-terminal",
                "nautilus",
                "gdm3",
            ],
            "kde" => vec![
                "kde-full",
                "sddm",
            ],
            "xfce" => vec![
                "xfce4",
                "xfce4-terminal",
                "lightdm",
            ],
            _ => vec![],
        }
    }
}

impl Feature for DesktopFeature {
    fn name(&self) -> &str {
        "desktop"
    }

    fn description(&self) -> &str {
        "Desktop environment support"
    }

    fn register_stages(&self, pipeline: &mut Vec<Box<dyn Stage>>) -> Result<()> {
        info!("Registering desktop feature stages");

        struct DesktopStage {
            packages: Vec<&'static str>,
        }

        impl Stage for DesktopStage {
            fn name(&self) -> &str {
                "desktop-install"
            }

            fn description(&self) -> &str {
                "Install desktop environment packages"
            }

            fn dependencies(&self) -> Vec<&str> {
                vec!["bootstrap"]
            }

            fn run(&self, _ctx: &mut BuildContext) -> Result<()> {
                info!("Installing desktop environment");
                
                debug!("Packages to install: {:?}", self.packages);
                
                Ok(())
            }
        }

        pipeline.push(Box::new(DesktopStage {
            packages: self.get_packages(),
        }));

        Ok(())
    }

    fn prepare_overlay(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Preparing desktop overlay");

        let overlay_base = match &ctx.workspace_layout {
            Some(layout) => layout.overlay.clone(),
            None => ctx.workspace.overlay.clone()
        };

        let overlay_dir = overlay_base.join("desktop");
        std::fs::create_dir_all(&overlay_dir)?;

        let branding_dir = overlay_dir.join("branding");
        std::fs::create_dir_all(&branding_dir)?;

        Ok(())
    }
}
