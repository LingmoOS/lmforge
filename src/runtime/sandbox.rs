use std::path::PathBuf;
use anyhow::{Result, bail};
use tracing::{info, warn};

use super::process::{Executor, ProcessConfig};

#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub root: PathBuf,
    pub mounts: Vec<MountPoint>,
    pub environment: Vec<(String, String)>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MountPoint {
    pub source: PathBuf,
    pub target: PathBuf,
    pub fs_type: Option<String>,
    pub options: Vec<String>,
}

impl SandboxConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        SandboxConfig {
            root: root.into(),
            mounts: vec![],
            environment: vec![],
            capabilities: vec![],
        }
    }

    pub fn with_mount(mut self, source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        self.mounts.push(MountPoint {
            source: source.into(),
            target: target.into(),
            fs_type: None,
            options: vec![],
        });
        self
    }

    pub fn with_bind_mount(mut self, source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        self.mounts.push(MountPoint {
            source: source.into(),
            target: target.into(),
            fs_type: Some("bind".to_string()),
            options: vec!["bind".to_string()],
        });
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.push((key.into(), value.into()));
        self
    }
}

pub struct Sandbox;

impl Sandbox {
    pub async fn prepare(config: &SandboxConfig) -> Result<()> {
        info!("Preparing sandbox at {:?}", config.root);

        for mount in &config.mounts {
            let target = config.root.join(&mount.target);
            
            if !target.exists() {
                std::fs::create_dir_all(&target)?;
            }

            match mount.fs_type.as_deref() {
                Some("bind") | None => {
                    Self::mount_bind(&mount.source, &target)?;
                }
                Some(fs_type) => {
                    Self::mount(&mount.source, &target, fs_type, &mount.options)?;
                }
            }
        }

        Ok(())
    }

    pub async fn cleanup(config: &SandboxConfig) -> Result<()> {
        info!("Cleaning up sandbox at {:?}", config.root);

        for mount in config.mounts.iter().rev() {
            let target = config.root.join(&mount.target);
            if let Err(e) = Self::unmount(&target) {
                warn!("Failed to unmount {:?}: {}", target, e);
            }
        }

        Ok(())
    }

    pub async fn execute_in(config: &SandboxConfig, command: &str, args: &[&str]) -> Result<String> {
        let mut full_args = vec![command.to_string()];
        full_args.extend(args.iter().map(|s| s.to_string()));

        let proc_config = ProcessConfig::new("chroot")
            .args(full_args)
            .working_dir(&config.root);

        for (key, value) in &config.environment {
            // Note: In real implementation, would need to handle environment properly
        }

        Executor::execute_success(&proc_config).await
    }
}
