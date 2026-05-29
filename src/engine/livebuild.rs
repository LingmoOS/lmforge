use std::path::{Path, PathBuf};
use anyhow::{Result, bail};
use tracing::{info, warn, debug, error};

use super::engine_trait::ImageEngine;
use crate::domain::context::BuildContext;
use crate::domain::artifact::{Artifact, ArtifactKind};
use crate::runtime::process::{Executor, ProcessConfig};
use crate::runtime::mount::Mount;
use crate::infra::workspace::WorkspaceLayout;

pub struct LiveBuildEngine {
    workspace: Option<WorkspaceLayout>,
}

impl LiveBuildEngine {
    pub fn new() -> Self {
        LiveBuildEngine {
            workspace: None,
        }
    }

    pub fn with_workspace(mut self, layout: WorkspaceLayout) -> Self {
        self.workspace = Some(layout);
        self
    }

    fn get_lb_config_dir(&self, ctx: &BuildContext) -> PathBuf {
        match &self.workspace {
            Some(ws) => ws.livebuild_config(),
            None => ctx.workspace.temp.join("live-build"),
        }
    }

    fn validate_prerequisites(&self) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        
        let lb_exists = rt.block_on(async { Executor::exists("lb").await });
        
        if !lb_exists {
            let lb_build_exists = rt.block_on(async { Executor::exists("lb_build").await });
            
            if !lb_build_exists {
                bail!(
                    "live-build is not installed.\n\n\
                     Installation:\n  sudo apt-get update\n  sudo apt-get install live-boot live-build\n\n\
                     Or use the lb_build wrapper if available."
                );
            }
        }

        info!(target: "lmforge_livebuild", "validated live-build prerequisites");
        Ok(())
    }

    fn generate_livebuild_config(&self, ctx: &BuildContext) -> Result<PathBuf> {
        let lb_config = self.get_lb_config_dir(ctx);
        
        info!(target: "lmforge_livebuild", config_dir = ?lb_config, "generating live-build configuration");
        
        std::fs::create_dir_all(&lb_config)?;
        
        self.write_auto_config(&lb_config, ctx)?;
        self.create_package_lists(&lb_config, ctx)?;
        self.setup_includes_chroot(&lb_config, ctx)?;
        self.create_hooks(&lb_config)?;

        debug!(target: "lmforge_livebuild", config_dir = ?lb_config, "configuration generated");

        Ok(lb_config)
    }

    fn write_auto_config(&self, dir: &Path, ctx: &BuildContext) -> Result<()> {
        let auto_dir = dir.join("auto");
        std::fs::create_dir_all(&auto_dir)?;

        let components = ctx.config.platform.components.join(" ");

        let config_content = format!(
            r#"#!/bin/sh
set -e

LB_ARCHITECTURE="{arch}"
LB_DISTRIBUTION="{suite}"
LB_ARCHIVE_AREAS="{components}"
LB_PARENT_ARCHIVE_AREAS="{components}"

LB_BOOTLOADER="grub-efi"
LB_CHROOT_FILESYSTEM="squashfs"
LB_BINARY_FILESYSTEM="fat32"
LB_BINARY_IMAGES="iso"

LB_ISO_APPLICATION="{app_name} {version}"
LB_ISO_PUBLISHER="{publisher}"
LB_ISO_VOLUME="{volume_id}"
"#,
            arch = ctx.arch(),
            suite = ctx.suite(),
            components = components,
            app_name = "Lingmo Linux",
            version = ctx.version(),
            publisher = "Lingmo Project",
            volume_id = ctx.config.image.volume_id,
        );

        std::fs::write(auto_dir.join("config"), config_content)?;
        debug!(target: "lmforge_livebuild", file = ?auto_dir.join("config"), "wrote auto/config");

        Ok(())
    }

    fn create_package_lists(&self, dir: &Path, _ctx: &BuildContext) -> Result<()> {
        let lists_dir = dir.join("config").join("package-lists");
        std::fs::create_dir_all(&lists_dir)?;

        let base_packages = r#"# Base system packages
linux-image-amd64
live-boot
systemd-sysv

# Desktop environment (if enabled)
task-xfce-desktop

# Network tools
network-manager
wpasupplicant
wireless-tools

# Filesystems support
dosfstools
ntfs-3g
exfat-fuse

# Utilities
vim-nox
curl
wget
git
"#;

        std::fs::write(lists_dir.join("base.list.chroot"), base_packages)?;

        if let Some(workspace_layout) = &self.workspace {
            let overlay_packages_path = workspace_layout.overlay.join("packages.list");
            
            if overlay_packages_path.exists() {
                let overlay_content = std::fs::read_to_string(&overlay_packages_path)?;
                std::fs::write(lists_dir.join("overlay.list.chroot"), overlay_content)?;
                debug!(target: "lmforge_livebuild", 
                    file = ?overlay_packages_path,
                    "included overlay package list"
                );
            }
        }

        Ok(())
    }

    fn setup_includes_chroot(&self, dir: &Path, _ctx: &BuildContext) -> Result<()> {
        let includes_chroot = dir.join("config").join("includes.chroot");
        std::fs::create_dir_all(includes_chroot.join("etc"))?;
        std::fs::create_dir_all(includes_chroot.join("usr/share"))?;

        let hostname_content = "lingmo-live\n";
        std::fs::write(includes_chroot.join("etc/hostname"), hostname_content)?;

        let hosts_content = r#"127.0.0.1	localhost
127.0.1.1	lingmo-live

# The following lines are desirable for IPv6 capable hosts
::1     localhost ip6-localhost ip6-loopback
fe00::0 ip6-localnet
ff00::0 ip6-mcastprefix
ff02::1 ip6-allnodes
ff02::2 ip6-allrouters
"#;
        std::fs::write(includes_chroot.join("etc/hosts"), hosts_content)?;

        Ok(())
    }

    fn create_hooks(&self, dir: &Path) -> Result<()> {
        let hooks_dir = dir.join("config").join("hooks");
        std::fs::create_dir_all(&hooks_dir)?;

        let post_build_hook = r#"#!/bin/sh
set -e

echo "Running lmforge post-build hook..."

# Add custom branding or modifications here
if [ -f /tmp/lmforge-branding ]; then
    echo "Applying branding..."
fi

echo "Post-build hook completed"
"#;

        std::fs::write(hooks_dir.join("999-lmforge-post.chroot"), post_build_hook)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let hook_path = hooks_dir.join("999-lmforge-post.chroot");
            let mut perms = std::fs::metadata(&hook_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_path, perms)?;
        }

        Ok(())
    }

    fn run_lb_command(&self, command: &str, args: &[&str], working_dir: &Path, timeout_secs: u64) -> Result<String> {
        let mut full_args = vec![command.to_string()];
        full_args.extend(args.iter().map(|s| s.to_string()));

        info!(
            target: "lmforge_livebuild",
            command = command,
            args = ?args,
            cwd = ?working_dir,
            timeout_secs = timeout_secs,
            "executing live-build command"
        );

        let config = ProcessConfig::new("lb")
            .arg(command)
            .args(args)
            .working_dir(working_dir)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .with_build_id("livebuild");

        let output = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::execute_success(&config).await
            })?
        };

        debug!(
            target: "lmforge_livebuild",
            command = command,
            stdout_len = output.len(),
            "command completed successfully"
        );

        Ok(output)
    }

    fn collect_artifacts(&self, ctx: &BuildContext, lb_output_dir: &Path) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        let iso_filename = format!("{}.iso", ctx.config.image.iso_name);
        let iso_source = lb_output_dir.join(&iso_filename);

        if iso_source.exists() {
            let mut artifact = Artifact::new(
                ArtifactKind::Iso,
                iso_source.clone(),
                ctx.arch(),
                ctx.suite(),
                ctx.version(),
            );

            let checksum = {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async { artifact.compute_checksum().await })?
            };

            let dest = match &self.workspace {
                Some(ws) => ws.artifact_output(&artifact.filename()),
                None => ctx.output_path().join(artifact.filename()),
            };

            let dest_clone = dest.clone();
            let iso_source_clone = iso_source.clone();

            {
                let rt = tokio::runtime::Runtime::new()?;
                let _bytes_copied = rt.block_on(async move {
                    tokio::fs::copy(&iso_source_clone, &dest_clone).await
                })?;
            }

            artifact.path = dest;
            artifacts.push(artifact);

            info!(
                target: "lmforge_livebuild",
                artifact = ?iso_filename,
                checksum = %checksum,
                "ISO artifact collected"
            );
        }

        let squashfs_path = lb_output_dir.join("binary/live/filesystem.squashfs");
        if squashfs_path.exists() {
            let mut artifact = Artifact::new(
                ArtifactKind::Squashfs,
                squashfs_path.clone(),
                ctx.arch(),
                ctx.suite(),
                ctx.version(),
            );

            {
                let rt = tokio::runtime::Runtime::new()?;
                let _checksum = rt.block_on(async { artifact.compute_checksum().await })?;
            }

            artifacts.push(artifact);

            info!(
                target: "lmforge_livebuild",
                artifact = "filesystem.squashfs",
                "SquashFS artifact collected"
            );
        }

        Ok(artifacts)
    }
}

impl Default for LiveBuildEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageEngine for LiveBuildEngine {
    fn name(&self) -> &str {
        "live-build"
    }

    fn prepare(&self, ctx: &mut BuildContext) -> Result<()> {
        info!(target: "lmforge_livebuild", stage = "prepare", "starting live-build preparation");

        self.validate_prerequisites()?;

        let lb_config = self.generate_livebuild_config(ctx)?;

        info!(target: "lmforge_livebuild", config_dir = ?lb_config, "running lb config");

        match self.run_lb_command("config", &[], &lb_config, 300) {
            Ok(output) => {
                debug!(target: "lmforge_livebuild", output = %output, "lb config completed");
            }
            Err(e) => {
                error!(target: "lmforge_livebuild", error = %e, "lb config failed");
                return Err(e);
            }
        }

        info!(target: "lmforge_livebuild", stage = "prepare", "live-build preparation completed");
        Ok(())
    }

    fn build(&self, ctx: &mut BuildContext) -> Result<Vec<Artifact>> {
        info!(target: "lmforge_livebuild", stage = "build", "starting ISO build with live-build");

        let lb_config = self.get_lb_config_dir(ctx);

        info!(target: "lmforge_livebuild", config_dir = ?lb_config, "running lb build (timeout: 3600s)");

        let output = self.run_lb_command("build", &[], &lb_config, 3600)?;

        info!(target: "lmforge_livebuild", output_len = output.len(), "lb build completed successfully");

        let lb_output_dir = lb_config;
        let artifacts = self.collect_artifacts(ctx, &lb_output_dir)?;

        for artifact in &artifacts {
            ctx.register_artifact(artifact.clone());
        }

        info!(
            target: "lmforge_livebuild",
            stage = "build",
            artifact_count = artifacts.len(),
            "ISO build completed"
        );

        Ok(artifacts)
    }

    fn cleanup(&self, ctx: &mut BuildContext) -> Result<()> {
        info!(target: "lmforge_livebuild", stage = "cleanup", "cleaning up live-build environment");

        let lb_config = self.get_lb_config_dir(ctx);

        if lb_config.exists() {
            info!(target: "lmforge_livebuild", config_dir = ?lb_config, "running lb clean --all");

            let clean_result = {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    Executor::execute(
                        &ProcessConfig::new("lb")
                            .arg("clean")
                            .arg("--all")
                            .working_dir(&lb_config)
                    ).await
                })
            };

            if let Err(e) = clean_result {
                warn!(target: "lmforge_livebuild", error = %e, "lb clean failed, will remove directory manually");
            }

            if let Err(e) = std::fs::remove_dir_all(&lb_config) {
                warn!(target: "lmforge_livebuild", error = %e, "failed to remove live-build config directory");
            } else {
                debug!(target: "lmforge_livebuild", config_dir = ?lb_config, "removed live-build config directory");
            }
        }

        let rootfs_path = match &ctx.workspace_layout {
            Some(layout) => layout.rootfs.clone(),
            None => ctx.workspace.rootfs.clone()
        };

        if rootfs_path.exists() {
            info!(target: "lmforge_livebuild", rootfs = ?rootfs_path, "unmounting rootfs mounts");

            let unmount_result = {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    Mount::unmount_all_from_chroot(&rootfs_path).await
                })
            };

            if let Err(e) = unmount_result {
                warn!(target: "lmforge_livebuild", error = %e, "failed to unmount all from chroot");
            }
        }

        info!(target: "lmforge_livebuild", stage = "cleanup", "cleanup completed");
        Ok(())
    }

    fn supported_formats(&self) -> Vec<&str> {
        vec!["iso", "netboot", "tar", "hdd"]
    }
}
