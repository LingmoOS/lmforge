use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::collections::VecDeque;
use anyhow::{Result, Context, bail};
use tracing::{info, debug, warn};

use super::process::{Executor, ProcessConfig};

#[derive(Debug, Clone)]
pub enum MountType {
    Proc,
    Sysfs,
    Devpts,
    Bind { source: PathBuf },
    Tmpfs { size: Option<String> },
}

impl std::fmt::Display for MountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountType::Proc => write!(f, "proc"),
            MountType::Sysfs => write!(f, "sysfs"),
            MountType::Devpts => write!(f, "devpts"),
            MountType::Bind { source } => write!(f, "bind({})", source.display()),
            MountType::Tmpfs { size } => {
                match size {
                    Some(s) => write!(f, "tmpfs({})", s),
                    None => write!(f, "tmpfs"),
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MountEntry {
    pub target: PathBuf,
    pub mount_type: MountType,
    pub mounted_at: chrono::DateTime<chrono::Utc>,
    pub build_id: String,
}

pub struct MountGuard {
    target: PathBuf,
    registry: Weak<Mutex<MountRegistryInner>>,
}

impl MountGuard {
    pub fn new(target: PathBuf, registry: &Arc<Mutex<MountRegistryInner>>) -> Self {
        MountGuard {
            target,
            registry: Arc::downgrade(registry),
        }
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    fn do_unmount(target: &Path) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        rt.block_on(async {
            let output = Executor::execute(
                &ProcessConfig::new("umount")
                    .arg(target)
            ).await?;

            match output.status {
                super::process::ExitStatus::Success => Ok(()),
                _ => {
                    debug!(
                        target: "lmforge_mount",
                        mount = ?target,
                        stderr = %output.stderr,
                        "normal unmount failed, trying lazy unmount"
                    );

                    let lazy_output = Executor::execute(
                        &ProcessConfig::new("umount")
                            .arg("-l")
                            .arg(target)
                    ).await?;

                    match lazy_output.status {
                        super::process::ExitStatus::Success => Ok(()),
                        _ => bail!("Failed to unmount {:?}: {}", target, lazy_output.stderr),
                    }
                }
            }
        })
    }
}

impl Drop for MountGuard {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            let reg = registry.lock().unwrap();
            
            if let Some(entry) = reg.mounts.iter().find(|m| m.target == self.target).cloned() {
                info!(
                    target: "lmforge_mount",
                    mount = %entry.target.display(),
                    mount_type = %entry.mount_type,
                    "MountGuard dropped, auto-unmounting"
                );

                drop(reg);
                
                match Self::do_unmount(&self.target) {
                    Ok(_) => {
                        if let Some(registry) = self.registry.upgrade() {
                            let mut reg = registry.lock().unwrap();
                            reg.mounts.retain(|m| m.target != self.target);
                        }
                        
                        debug!(
                            target: "lmforge_mount",
                            mount = %self.target.display(),
                            "auto-unmount successful"
                        );
                    }
                    Err(e) => {
                        warn!(
                            target: "lmforge_mount",
                            mount = %self.target.display(),
                            error = %e,
                            "auto-unmount failed, will retry in cleanup"
                        );
                    }
                }
            }
        }
    }
}

struct MountRegistryInner {
    mounts: VecDeque<MountEntry>,
    build_id: String,
}

pub struct MountRegistry {
    inner: Arc<Mutex<MountRegistryInner>>,
}

impl MountRegistry {
    pub fn new(build_id: &str) -> Self {
        MountRegistry {
            inner: Arc::new(Mutex::new(MountRegistryInner {
                mounts: VecDeque::new(),
                build_id: build_id.to_string(),
            })),
        }
    }

    pub fn register(&self, entry: MountEntry) -> Result<MountGuard> {
        let mut inner = self.inner.lock().unwrap();
        
        debug!(
            target: "lmforge_mount",
            mount = %entry.target.display(),
            mount_type = %entry.mount_type,
            "registering mount point"
        );
        
        inner.mounts.push_back(entry.clone());
        
        Ok(MountGuard::new(entry.target.clone(), &self.inner))
    }

    pub fn unregister(&self, target: &Path) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let len_before = inner.mounts.len();
        
        inner.mounts.retain(|m| m.target != target);
        
        let removed = inner.mounts.len() < len_before;
        
        if removed {
            debug!(
                target: "lmforge_mount",
                mount = ?target,
                "unregistered mount point"
            );
        }
        
        removed
    }

    pub fn get_all(&self) -> Vec<MountEntry> {
        let inner = self.inner.lock().unwrap();
        inner.mounts.iter().cloned().collect()
    }

    pub fn get_reverse_order(&self) -> Vec<MountEntry> {
        let inner = self.inner.lock().unwrap();
        inner.mounts.iter().rev().cloned().collect()
    }

    pub fn count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.mounts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    pub fn contains(&self, target: &Path) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.mounts.iter().any(|m| m.target == target)
    }

    pub fn find_by_target(&self, target: &Path) -> Option<MountEntry> {
        let inner = self.inner.lock().unwrap();
        inner.mounts.iter().find(|m| m.target == target).cloned()
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.mounts.clear();
        debug!(target: "lmforge_mount", "cleared all registered mounts");
    }
}

impl Clone for MountRegistry {
    fn clone(&self) -> Self {
        MountRegistry {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub struct MountManager {
    registry: MountRegistry,
    build_id: String,
}

impl MountManager {
    pub fn new(build_id: &str) -> Self {
        MountManager {
            registry: MountRegistry::new(build_id),
            build_id: build_id.to_string(),
        }
    }

    pub fn registry(&self) -> &MountRegistry {
        &self.registry
    }

    pub async fn mount_proc(&self, root: &Path) -> Result<MountGuard> {
        let target = root.join("proc");
        
        std::fs::create_dir_all(&target)?;
        
        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("proc")
                .arg("proc")
                .arg(&target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                let entry = MountEntry {
                    target: target.clone(),
                    mount_type: MountType::Proc,
                    mounted_at: chrono::Utc::now(),
                    build_id: self.build_id.clone(),
                };
                
                let guard = self.registry.register(entry)?;
                
                info!(
                    target: "lmforge_mount",
                    mount = %target.display(),
                    mount_type = "proc",
                    "mounted successfully (RAII guarded)"
                );
                
                Ok(guard)
            }
            _ => bail!("Failed to mount proc on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn mount_sysfs(&self, root: &Path) -> Result<MountGuard> {
        let target = root.join("sys");
        
        std::fs::create_dir_all(&target)?;
        
        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("sysfs")
                .arg("sysfs")
                .arg(&target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                let entry = MountEntry {
                    target: target.clone(),
                    mount_type: MountType::Sysfs,
                    mounted_at: chrono::Utc::now(),
                    build_id: self.build_id.clone(),
                };
                
                let guard = self.registry.register(entry)?;
                
                info!(
                    target: "lmforge_mount",
                    mount = %target.display(),
                    mount_type = "sysfs",
                    "mounted successfully (RAII guarded)"
                );
                
                Ok(guard)
            }
            _ => bail!("Failed to mount sysfs on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn mount_devpts(&self, root: &Path) -> Result<MountGuard> {
        let target = root.join("dev/pts");
        
        std::fs::create_dir_all(&target)?;
        
        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("devpts")
                .arg("devpts")
                .arg(&target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                let entry = MountEntry {
                    target: target.clone(),
                    mount_type: MountType::Devpts,
                    mounted_at: chrono::Utc::now(),
                    build_id: self.build_id.clone(),
                };
                
                let guard = self.registry.register(entry)?;
                
                info!(
                    target: "lmforge_mount",
                    mount = %target.display(),
                    mount_type = "devpts",
                    "mounted successfully (RAII guarded)"
                );
                
                Ok(guard)
            }
            _ => bail!("Failed to mount devpts on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn mount_bind(&self, source: &Path, root: &Path, dest: &Path) -> Result<MountGuard> {
        let target = root.join(dest);
        
        if !source.exists() {
            bail!("Source path does not exist: {:?}", source);
        }

        std::fs::create_dir_all(&target)?;
        
        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("--bind")
                .arg(source)
                .arg(&target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                let entry = MountEntry {
                    target: target.clone(),
                    mount_type: MountType::Bind { source: source.to_path_buf() },
                    mounted_at: chrono::Utc::now(),
                    build_id: self.build_id.clone(),
                };
                
                let guard = self.registry.register(entry)?;
                
                info!(
                    target: "lmforge_mount",
                    source = %source.display(),
                    mount = %target.display(),
                    mount_type = "bind",
                    "mounted successfully (RAII guarded)"
                );
                
                Ok(guard)
            }
            _ => bail!("Failed to bind mount {:?} to {:?}: {}", source, target, output.stderr),
        }
    }

    pub async fn mount_tmpfs(&self, root: &Path, dest: &Path, size: Option<&str>) -> Result<MountGuard> {
        let target = root.join(dest);
        let size_str = size.unwrap_or("100M").to_string();
        
        std::fs::create_dir_all(&target)?;
        
        let output = Executor::execute(
            &ProcessConfig::new("mount")
                .arg("-t")
                .arg("tmpfs")
                .arg("-o")
                .arg(format!("size={}", size_str))
                .arg("tmpfs")
                .arg(&target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                let entry = MountEntry {
                    target: target.clone(),
                    mount_type: MountType::Tmpfs { size: Some(size_str.clone()) },
                    mounted_at: chrono::Utc::now(),
                    build_id: self.build_id.clone(),
                };
                
                let guard = self.registry.register(entry)?;
                
                info!(
                    target: "lmforge_mount",
                    mount = %target.display(),
                    mount_type = %format!("tmpfs({})", size_str),
                    "mounted successfully (RAII guarded)"
                );
                
                Ok(guard)
            }
            _ => bail!("Failed to mount tmpfs on {:?}: {}", target, output.stderr),
        }
    }

    pub async fn mount_all_for_chroot(&self, root: &Path) -> Result<Vec<MountGuard>> {
        info!(
            target: "lmforge_mount",
            root = ?root,
            "mounting all filesystems for chroot (RAII guarded)"
        );

        let mut guards = Vec::new();

        guards.push(self.mount_proc(root).await?);
        guards.push(self.mount_sysfs(root).await?);
        guards.push(self.mount_devpts(root).await?);
        guards.push(self.mount_bind(Path::new("/dev"), root, Path::new("dev")).await?);
        guards.push(self.mount_tmpfs(root, Path::new("run"), Some("100M")).await?);

        info!(
            target: "lmforge_mount",
            count = guards.len(),
            root = ?root,
            "all filesystems mounted with RAII guards"
        );

        Ok(guards)
    }

    pub async fn unmount_all_reverse(&self) -> Result<Vec<Result<()>>> {
        let mounts = self.registry.get_reverse_order();
        
        info!(
            target: "lmforge_mount",
            count = mounts.len(),
            "unmounting all mounts in reverse order"
        );

        let mut results = Vec::new();

        for entry in &mounts {
            debug!(
                target: "lmforge_mount",
                mount = %entry.target.display(),
                mount_type = %entry.mount_type,
                "unmounting in reverse order"
            );

            let result = Self::unmount_single(&entry.target).await;
            
            if result.is_ok() {
                self.registry.unregister(&entry.target);
            }
            
            results.push(result);
        }

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let fail_count = results.len() - success_count;

        if fail_count > 0 {
            warn!(
                target: "lmforge_mount",
                success = success_count,
                failed = fail_count,
                total = results.len(),
                "{} mounts failed to unmount",
                fail_count
            );
        } else {
            info!(
                target: "lmforge_mount",
                count = success_count,
                "all mounts unmounted successfully"
            );
        }

        Ok(results)
    }

    async fn unmount_single(target: &Path) -> Result<()> {
        if !target.exists() {
            debug!(
                target: "lmforge_mount",
                mount = ?target,
                "mount point does not exist, skipping"
            );
            return Ok(());
        }

        let output = Executor::execute(
            &ProcessConfig::new("umount")
                .arg(target)
        ).await?;

        match output.status {
            super::process::ExitStatus::Success => {
                debug!(
                    target: "lmforge_mount",
                    mount = ?target,
                    "unmounted successfully"
                );
                Ok(())
            }
            _ => {
                debug!(
                    target: "lmforge_mount",
                    mount = ?target,
                    stderr = %output.stderr,
                    "normal unmount failed, trying lazy unmount"
                );
                
                let lazy_output = Executor::execute(
                    &ProcessConfig::new("umount")
                        .arg("-l")
                        .arg(target)
                ).await?;

                match lazy_output.status {
                    super::process::ExitStatus::Success => {
                        debug!(
                            target: "lmforge_mount",
                            mount = ?target,
                            "lazy unmount successful"
                        );
                        Ok(())
                    }
                    _ => bail!("Failed to unmount {:?} (tried normal and lazy): {}", target, lazy_output.stderr),
                }
            }
        }
    }

    pub fn force_cleanup_all(&self) -> Result<()> {
        info!(target: "lmforge_mount", "force cleaning up all mounts");

        let mounts = self.registry.get_reverse_order();

        for entry in &mounts {
            if let Err(e) = Self::force_unmount_sync(&entry.target) {
                warn!(
                    target: "lmforge_mount",
                    mount = %entry.target.display(),
                    error = %e,
                    "force unmount failed"
                );
            } else {
                self.registry.unregister(&entry.target);
            }
        }

        self.registry.clear();

        info!(target: "lmforge_mount", "force cleanup completed");
        Ok(())
    }

    fn force_unmount_sync(target: &Path) -> Result<()> {
        if !target.exists() {
            return Ok(());
        }

        let result = std::process::Command::new("umount")
            .arg("-l")  
            .arg(target)
            .output()
            .context(format!("Failed to execute umount for {:?}", target))?;

        if result.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&result.stderr);
            bail!("Force unmount failed for {:?}: {}", target, stderr);
        }
    }
}
