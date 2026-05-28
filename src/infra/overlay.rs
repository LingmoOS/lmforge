use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use tracing::{info, debug, warn};

use crate::infra::workspace::WorkspaceLayout;

pub struct OverlayManager {
    workspace: Option<WorkspaceLayout>,
    rootfs_path: Option<PathBuf>,
}

impl OverlayManager {
    pub fn new(workspace: &WorkspaceLayout) -> Self {
        OverlayManager {
            workspace: Some(workspace.clone()),
            rootfs_path: None,
        }
    }

    pub fn new_for_rootfs(rootfs: &Path) -> Self {
        OverlayManager {
            workspace: None,
            rootfs_path: Some(rootfs.to_path_buf()),
        }
    }

    pub fn initialize(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available. Use new_for_rootfs for simple operations."))?;

        info!(target: "lmforge_overlay", "initializing overlay directories");

        let dirs = [
            ("filesystem", workspace.overlay.join("filesystem")),
            ("branding", workspace.overlay.join("branding")),
            ("hooks", workspace.overlay.join("hooks")),
        ];

        for (name, path) in &dirs {
            std::fs::create_dir_all(path)
                .with_context(|| format!("Failed to create {} overlay directory: {:?}", name, path))?;
            
            debug!(target: "lmforge_overlay", directory = ?path, name = name, "created overlay directory");
        }

        self.create_default_branding()?;
        
        info!(target: "lmforge_overlay", "overlay directories initialized");
        Ok(())
    }

    fn create_default_branding(&self) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let branding_dir = workspace.overlay.join("branding");

        let issue_content = r#"Lingmo Linux Live \n \l
"#;
        std::fs::write(branding_dir.join("etc/issue"), issue_content)?;

        let issue_net_content = r#"Lingmo Linux Live (\n) (\l)
"#;
        std::fs::write(branding_dir.join("etc/issue.net"), issue_net_content)?;

        let os_release_content = r#"PRETTY_NAME="Lingmo Linux"
NAME="Lingmo Linux"
VERSION_ID=1.0
VERSION="1.0 (Live)"
ID=lingmo
ID_LIKE=debian
HOME_URL="https://www.lingmo.org"
SUPPORT_URL="https://www.lingmo.org/support"
BUG_REPORT_URL="https://bugs.lingmo.org"
"#;
        std::fs::write(branding_dir.join("etc/os-release"), os_release_content)?;

        debug!(target: "lmforge_overlay", "created default branding files");

        Ok(())
    }

    pub fn apply_to_livebuild(&self, lb_config: &Path) -> Result<()> {
        info!(
            target: "lmforge_overlay",
            lb_config = ?lb_config,
            "applying overlays to live-build configuration"
        );

        let includes_chroot = lb_config.join("config").join("includes.chroot");
        std::fs::create_dir_all(&includes_chroot)?;

        self.apply_filesystem_overlay(&includes_chroot)?;
        self.apply_branding_overlay(&includes_chroot)?;
        self.copy_package_list(lb_config)?;
        self.install_hooks(lb_config)?;

        info!(target: "lmforge_overlay", "overlays applied to live-build configuration");
        Ok(())
    }

    pub fn apply_overlays(&self, rootfs: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available for apply_overlays"))?;

        info!(
            target: "lmforge_overlay",
            overlay_name = "rootfs",
            target = %rootfs.display(),
            "applying overlays to rootfs"
        );

        if workspace.overlay.join("filesystem").exists() {
            self.copy_recursive(&workspace.overlay.join("filesystem"), rootfs)?;
            debug!(target: "lmforge_overlay", "copied filesystem overlay");
        }

        Ok(())
    }

    fn apply_filesystem_overlay(&self, target: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let filesystem_source = workspace.overlay.join("filesystem");

        if !filesystem_source.exists() {
            debug!(target: "lmforge_overlay", "no filesystem overlay found, skipping");
            return Ok(());
        }

        info!(
            target: "lmforge_overlay",
            source = ?filesystem_source,
            target = ?target,
            "applying filesystem overlay"
        );

        self.copy_recursive(&filesystem_source, target)?;

        Ok(())
    }

    fn apply_branding_overlay(&self, target: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let branding_source = workspace.overlay.join("branding");

        if !branding_source.exists() {
            debug!(target: "lmforge_overlay", "no branding overlay found, skipping");
            return Ok(());
        }

        info!(
            target: "lmforge_overlay",
            source = ?branding_source,
            target = ?target,
            "applying branding overlay"
        );

        self.merge_directory(&branding_source, target)?;

        Ok(())
    }

    fn copy_package_list(&self, lb_config: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let packages_list_path = workspace.overlay.join("packages.list");
        
        if !packages_list_path.exists() {
            debug!(target: "lmforge_overlay", "no custom package list found, skipping");
            return Ok(());
        }

        let lists_dir = lb_config.join("config").join("package-lists");
        std::fs::create_dir_all(&lists_dir)?;

        let content = std::fs::read_to_string(&packages_list_path)
            .context("Failed to read packages.list")?;

        if content.trim().is_empty() {
            debug!(target: "lmforge_overlay", "package list is empty, skipping");
            return Ok(());
        }

        let dest_file = lists_dir.join("custom-packages.list.chroot");
        std::fs::write(&dest_file, content)
            .with_context(|| format!("Failed to write package list to {:?}", dest_file))?;

        info!(
            target: "lmforge_overlay",
            file = ?packages_list_path,
            dest = ?dest_file,
            "copied custom package list"
        );

        Ok(())
    }

    fn install_hooks(&self, lb_config: &Path) -> Result<()> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceLayout not available"))?;

        let hooks_source = workspace.overlay.join("hooks");

        if !hooks_source.exists() {
            debug!(target: "lmforge_overlay", "no custom hooks found, skipping");
            return Ok(());
        }

        let hooks_dest = lb_config.join("config").join("hooks");
        std::fs::create_dir_all(&hooks_dest)?;

        for entry in std::fs::read_dir(&hooks_source)? {
            let entry = entry?;
            let source_path = entry.path();
            
            if source_path.is_file() && source_path.extension().map(|e| e == "chroot").unwrap_or(false) {
                let filename = source_path.file_name()
                    .context("Invalid hook filename")?
                    .to_string_lossy()
                    .to_string();
                
                let dest_path = hooks_dest.join(&filename);
                
                std::fs::copy(&source_path, &dest_path)
                    .with_context(|| format!("Failed to copy hook {:?} to {:?}", source_path, dest_path))?;

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&dest_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dest_path, perms)?;
                }

                debug!(
                    target: "lmforge_overlay",
                    hook = %filename,
                    dest = ?dest_path,
                    "installed hook"
                );
            }
        }

        info!(target: "lmforge_overlay", "hooks installed");
        Ok(())
    }

    pub fn load_package_list(&self) -> Result<Vec<String>> {
        let pkg_file = match &self.workspace {
            Some(ws) => ws.overlay.join("packages.list"),
            None => {
                let rootfs = self.rootfs_path.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No workspace or rootfs path available"))?;
                rootfs.join("..").join("overlay").join("packages.list")
            }
        };
        
        if !pkg_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&pkg_file)?;
        let packages: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        debug!(
            target: "lmforge_overlay",
            count = packages.len(),
            "loaded custom package list"
        );

        Ok(packages)
    }

    fn copy_recursive(&self, source: &Path, target: &Path) -> Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let src_path = entry.path();
            let rel_path = src_path.strip_prefix(source)?;
            let dst_path = target.join(rel_path);

            if src_path.is_dir() {
                std::fs::create_dir_all(&dst_path)?;
                self.copy_recursive(&src_path, &dst_path)?;
            } else if src_path.is_file() {
                if let Some(parent) = dst_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                std::fs::copy(&src_path, &dst_path)
                    .with_context(|| format!("Failed to copy {:?} to {:?}", src_path, dst_path))?;
                
                debug!(
                    target: "lmforge_overlay",
                    source = ?src_path,
                    dest = ?dst_path,
                    "copied file"
                );
            }
        }

        Ok(())
    }

    fn merge_directory(&self, source: &Path, target: &Path) -> Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let src_path = entry.path();
            let rel_path = src_path.strip_prefix(source)?;
            let dst_path = target.join(rel_path);

            if src_path.is_dir() {
                std::fs::create_dir_all(&dst_path)?;
                self.merge_directory(&src_path, &dst_path)?;
            } else if src_path.is_file() {
                if let Some(parent) = dst_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                if dst_path.exists() {
                    warn!(
                        target: "lmforge_overlay",
                        file = ?dst_path,
                        "overwriting existing file with branding version"
                    );
                }

                std::fs::copy(&src_path, &dst_path)
                    .with_context(|| format!("Failed to merge {:?} into {:?}", src_path, dst_path))?;
                
                debug!(
                    target: "lmforge_overlay",
                    source = ?src_path,
                    dest = ?dst_path,
                    "merged file"
                );
            }
        }

        Ok(())
    }
}
