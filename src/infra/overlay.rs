use std::path::{Path, PathBuf};
use anyhow::Result;
use tracing::{info, debug};

pub struct OverlayConfig {
    pub path: PathBuf,
    pub name: String,
}

pub struct OverlayManager {
    overlays: Vec<OverlayConfig>,
    workspace: PathBuf,
}

impl OverlayManager {
    pub fn new(workspace: &Path) -> Self {
        OverlayManager {
            overlays: Vec::new(),
            workspace: workspace.to_path_buf(),
        }
    }

    pub fn add_overlay(&mut self, path: &Path, name: &str) {
        self.overlays.push(OverlayConfig {
            path: path.to_path_buf(),
            name: name.to_string(),
        });
    }

    pub fn apply_overlays(&self, rootfs: &Path) -> Result<()> {
        for overlay in &self.overlays {
            info!(
                overlay_name = %overlay.name,
                target = %rootfs.display(),
                "applying overlay"
            );

            self.apply_single_overlay(&overlay.path, rootfs)?;
        }

        Ok(())
    }

    fn apply_single_overlay(&self, source: &Path, target: &Path) -> Result<()> {
        if source.join("filesystem").exists() {
            self.copy_directory(&source.join("filesystem"), target)?;
            debug!(source = %source.join("filesystem").display(), "copied filesystem overlay");
        }

        Ok(())
    }

    fn copy_directory(&self, source: &Path, target: &Path) -> Result<()> {
        let _ = (source, target);
        Ok(())
    }

    pub fn load_package_list(workspace: &Path) -> Result<Vec<String>> {
        let pkg_file = workspace.join("packages.list");
        
        if !pkg_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&pkg_file)?;
        let packages: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        Ok(packages)
    }
}
