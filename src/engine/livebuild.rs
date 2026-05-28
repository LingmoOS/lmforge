use std::path::{Path, PathBuf};
use anyhow::{Result, bail};
use tracing::{info, warn};

use super::engine_trait::ImageEngine;
use crate::domain::context::BuildContext;
use crate::domain::artifact::{Artifact, ArtifactKind};
use crate::runtime::{process::{Executor, ProcessConfig}, mount::Mount};

pub struct LiveBuildEngine {
    config_dir: PathBuf,
}

impl LiveBuildEngine {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        LiveBuildEngine {
            config_dir: config_dir.into(),
        }
    }

    fn generate_live_build_config(&self, ctx: &BuildContext) -> Result<PathBuf> {
        let lb_config = ctx.workspace.temp.join("live-build");
        
        std::fs::create_dir_all(&lb_config)?;
        
        self.write_auto_config(&lb_config, ctx)?;
        self.write_config_files(&lb_config, ctx)?;

        info!("Live-build config generated at {:?}", lb_config);
        Ok(lb_config)
    }

    fn write_auto_config(&self, dir: &Path, ctx: &BuildContext) -> Result<()> {
        let auto_dir = dir.join("auto");
        std::fs::create_dir_all(&auto_dir)?;

        let config_content = format!(
            r#"#!/bin/sh
set -e

LB_ARCHITECTURE="{}"
LB_DISTRIBUTION="{}"
LB_ARCHIVE_AREAS="{}"
LB_PARENT_ARCHIVE_AREAS="{}"

LB_BOOTLOADER="grub-efi"
LB_CHROOT_FILESYSTEM="squashfs"
LB_BINARY_FILESYSTEM="fat32"
LB_BINARY_IMAGES="iso"

LB_ISO_APPLICATION="Lingmo Linux {}"
LB_ISO_PUBLISHER="Lingmo Project"
LB_ISO_VOLUME="{}"
"#,
            ctx.arch(),
            ctx.suite(),
            ctx.config.platform.components.join(" "),
            ctx.config.platform.components.join(" "),
            ctx.version(),
            ctx.config.image.volume_id
        );

        std::fs::write(auto_dir.join("config"), config_content)?;

        Ok(())
    }

    fn write_config_files(&self, dir: &Path, _ctx: &BuildContext) -> Result<()> {
        let config_dir = dir.join("config");

        std::fs::create_dir_all(config_dir.join("bootloaders"))?;
        std::fs::create_dir_all(config_dir.join("includes.chroot"))?;
        std::fs::create_dir_all(config_dir.join("hooks"))?;

        let lists_dir = config_dir.join("package-lists");
        std::fs::create_dir_all(&lists_dir)?;

        Ok(())
    }
}

impl ImageEngine for LiveBuildEngine {
    fn name(&self) -> &str {
        "live-build"
    }

    fn prepare(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Preparing live-build environment");

        let exists = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::exists("lb").await || Executor::exists("lb_build").await
            })
        };

        if !exists {
            bail!(
                "live-build is not installed. Please install it first:\n  apt-get install live-build"
            );
        }

        self.generate_live_build_config(ctx)?;

        let lb_config = ctx.workspace.temp.join("live-build");
        let lb_config_clone = lb_config.clone();

        {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move {
                Executor::execute_success(
                    &ProcessConfig::new("lb")
                        .arg("config")
                        .working_dir(&lb_config_clone)
                ).await
            })?;
        }

        info!("Live-build preparation completed");
        Ok(())
    }

    fn build(&self, ctx: &mut BuildContext) -> Result<Vec<Artifact>> {
        info!("Building image with live-build");

        let lb_config = ctx.workspace.temp.join("live-build");

        let output = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::execute(
                    &ProcessConfig::new("lb")
                        .arg("build")
                        .working_dir(&lb_config)
                        .timeout(std::time::Duration::from_secs(3600))
                ).await
            })?
        };

        match output.status {
            crate::runtime::process::ExitStatus::Success => {
                info!("Live-build completed successfully");
                
                let mut artifacts = Vec::new();
                
                let iso_path = lb_config.join(format!("{}.iso", ctx.config.image.iso_name));
                if iso_path.exists() {
                    let mut artifact = Artifact::new(
                        ArtifactKind::Iso,
                        iso_path.clone(),
                        ctx.arch(),
                        ctx.suite(),
                        ctx.version(),
                    );
                    
                    let _checksum = {
                        let rt = tokio::runtime::Runtime::new()?;
                        rt.block_on(async {
                            artifact.compute_checksum().await
                        })?
                    };
                    
                    let output_iso = ctx.output_path().join(artifact.filename());
                    let output_iso_clone = output_iso.clone();
                    
                    {
                        let rt = tokio::runtime::Runtime::new()?;
                        rt.block_on(async move {
                            tokio::fs::copy(&iso_path, &output_iso_clone).await
                        })?
                    };
                    
                    artifact.path = output_iso;
                    artifacts.push(artifact);
                }

                let squashfs_path = lb_config.join("binary/live/filesystem.squashfs");
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
                        rt.block_on(async {
                            artifact.compute_checksum().await
                        })?
                    };
                    artifacts.push(artifact);
                }

                for artifact in &artifacts {
                    ctx.register_artifact(artifact.clone());
                }

                Ok(artifacts)
            }
            crate::runtime::process::ExitStatus::Timeout => {
                bail!("Live-build timed out after 1 hour");
            }
            crate::runtime::process::ExitStatus::Failure(code) => {
                bail!(
                    "Live-build failed with exit code {}:\nstderr: {}",
                    code,
                    output.stderr
                );
            }
            _ => {
                bail!("Live-build failed with unknown error");
            }
        }
    }

    fn cleanup(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Cleaning up live-build environment");

        let lb_config = ctx.workspace.temp.join("live-build");
        
        if lb_config.exists() {
            let result = {
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
            
            if let Err(e) = result {
                warn!("Failed to run lb clean: {}", e);
            }
            
            if let Err(e) = std::fs::remove_dir_all(&lb_config) {
                warn!("Failed to remove live-build config directory: {}", e);
            }
        }

        {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Mount::unmount_all_from_chroot(&ctx.workspace.rootfs).await
            })?
        }

        Ok(())
    }

    fn supported_formats(&self) -> Vec<&str> {
        vec!["iso", "netboot", "tar", "hdd"]
    }
}
