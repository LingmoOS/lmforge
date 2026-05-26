use std::path::Path;
use anyhow::{Result, bail};
use tracing::{debug, warn};

use super::process::{Executor, ProcessConfig};

pub struct Mount;

impl Mount {
    pub async fn bind(source: &Path, target: &Path) -> Result<()> {
        debug!("Binding mount {:?} -> {:?}", source, target);
        
        if !source.exists() {
            bail!("Source path does not exist: {:?}", source);
        }

        std::fs::create_dir_all(target)?;

        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("--bind")
                .arg(source)
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => bail!("Failed to bind mount {:?} to {:?}: {}", source, target, output.stderr),
        }
    }

    pub async fn unmount(target: &Path) -> Result<()> {
        debug!("Unmounting {:?}", target);

        let output = Executor::execute(
            &ProcessConfig::new("umount")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => {
                warn!("Failed to unmount {:?}: {}, trying lazy unmount", target, output.stderr);
                
                let lazy_output = Executor::execute(
                    &ProcessConfig::new("umount")
                        .arg("-l")
                        .arg(target)
                ).await?;

                match lazy_output.status {
                    super::process::ExitStatus::Success => Ok(()),
                    _ => bail!("Failed to lazy unmount {:?}: {}", target, lazy_output.stderr),
                }
            }
        }
    }

    pub async fn tmpfs(target: &Path, size: Option<&str>) -> Result<()> {
        let size_str = size.unwrap_or("1G");
        
        debug!("Mounting tmpfs on {:?} (size={})", target, size_str);

        std::fs::create_dir_all(target)?;

        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("tmpfs")
                .arg("-o")
                .arg(format!("size={}", size_str))
                .arg("tmpfs")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => bail!("Failed to mount tmpfs on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn proc(target: &Path) -> Result<()> {
        debug!("Mounting proc on {:?}", target);

        std::fs::create_dir_all(target)?;

        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("proc")
                .arg("proc")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => bail!("Failed to mount proc on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn sysfs(target: &Path) -> Result<()> {
        debug!("Mounting sysfs on {:?}", target);

        std::fs::create_dir_all(target)?;

        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("sysfs")
                .arg("sysfs")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => bail!("Failed to mount sysfs on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn devpts(target: &Path) -> Result<()> {
        debug!("Mounting devpts on {:?}", target);

        std::fs::create_dir_all(target)?;

        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("devpts")
                .arg("devpts")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => Ok(()),
            _ => bail!("Failed to mount devpts on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn mount_all_for_chroot(root: &Path) -> Result<()> {
        info!("Mounting all filesystems for chroot at {:?}", root);

        Self::proc(&root.join("proc")).await?;
        Self::sysfs(&root.join("sys")).await?;
        Self::devpts(&root.join("dev/pts")).await?;
        Self::bind(Path::new("/dev"), &root.join("dev")).await?;
        Self::tmpfs(&root.join("run"), Some("100M")).await?;

        Ok(())
    }

    pub async fn unmount_all_from_chroot(root: &Path) -> Result<()> {
        info!("Unmounting all filesystems from chroot at {:?}", root);

        let mount_points = [
            root.join("run"),
            root.join("dev/pts"),
            root.join("dev"),
            root.join("sys"),
            root.join("proc"),
        ];

        for mount_point in mount_points.iter().rev() {
            if mount_point.exists() {
                if let Err(e) = Self::unmount(mount_point).await {
                    warn!("Failed to unmount {:?}: {}", mount_point, e);
                }
            }
        }

        Ok(())
    }
}
