use std::path::Path;
use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};
use chrono::Utc;

use crate::infra::workspace::{WorkspaceManager, WorkspaceLayout, InterruptedBuild};
use crate::runtime::{process::{Executor, ProcessConfig}};

pub struct CleanupRecovery {
    workspace_manager: WorkspaceManager,
    workspace_layout: Option<WorkspaceLayout>,
}

impl CleanupRecovery {
    pub fn new(workspace_manager: WorkspaceManager) -> Self {
        CleanupRecovery {
            workspace_manager,
            workspace_layout: None,
        }
    }

    pub fn with_workspace(mut self, layout: WorkspaceLayout) -> Self {
        self.workspace_layout = Some(layout);
        self
    }

    pub fn initialize(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "initializing cleanup and recovery system");

        self.cleanup_stale_workspaces()?;
        self.detect_interrupted_builds()?;

        if let Some(layout) = &self.workspace_layout {
            self.create_lock_file(layout)?;
            self.create_pid_file(layout)?;
        }

        info!(target: "lmforge_cleanup", "cleanup and recovery system initialized");
        Ok(())
    }

    fn create_lock_file(&self, layout: &WorkspaceLayout) -> Result<()> {
        let lock_file = layout.root.join(".build.lock");

        let lock_content = format!(
            r#"build_id: {build_id}
pid: {pid}
started_at: {timestamp}
status: in_progress
"#,
            build_id = self.workspace_manager.build_id(),
            pid = std::process::id(),
            timestamp = Utc::now().to_rfc3339(),
        );

        std::fs::write(&lock_file, lock_content)
            .with_context(|| format!("Failed to create lock file: {:?}", lock_file))?;

        debug!(target: "lmforge_cleanup", file = ?lock_file, "created build lock file");

        Ok(())
    }

    fn create_pid_file(&self, layout: &WorkspaceLayout) -> Result<()> {
        let pid_file = layout.root.join(".build.pid");

        std::fs::write(
            &pid_file,
            format!("{}", std::process::id())
        ).with_context(|| format!("Failed to create PID file: {:?}", pid_file))?;

        debug!(target: "lmforge_cleanup", file = ?pid_file, "created PID file");

        Ok(())
    }

    pub fn cleanup_stale_workspaces(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "checking for stale workspaces");

        let cleaned = self.workspace_manager.cleanup_stale_workspaces(7)?;

        if !cleaned.is_empty() {
            info!(
                target: "lmforge_cleanup",
                count = cleaned.len(),
                "cleaned {} stale workspaces",
                cleaned.len()
            );
        } else {
            debug!(target: "lmforge_cleanup", "no stale workspaces found");
        }

        Ok(())
    }

    pub fn detect_interrupted_builds(&self) -> Result<Vec<InterruptedBuild>> {
        info!(target: "lmforge_cleanup", "scanning for interrupted builds");

        let interrupted = self.workspace_manager.detect_interrupted_builds()?;

        if !interrupted.is_empty() {
            warn!(
                target: "lmforge_cleanup",
                count = interrupted.len(),
                "found {} interrupted builds",
                interrupted.len()
            );

            for build in &interrupted {
                warn!(
                    target: "lmforge_cleanup",
                    path = ?build.path,
                    has_lock = build.has_lock_file,
                    has_pid = build.has_pid_file,
                    "interrupted build detected"
                );
            }
        } else {
            debug!(target: "lmforge_cleanup", "no interrupted builds found");
        }

        Ok(interrupted)
    }

    pub fn recover_interrupted_build(&self, build_path: &Path) -> Result<RecoveryResult> {
        info!(
            target: "lmforge_cleanup",
            path = ?build_path,
            "attempting to recover interrupted build"
        );

        let lock_file = build_path.join(".build.lock");
        
        if !lock_file.exists() {
            return Err(anyhow::anyhow!(
                "No lock file found at {:?}. Cannot recover.",
                build_path
            ));
        }

        let lock_content = std::fs::read_to_string(&lock_file)?;
        let status = parse_build_status(&lock_content);

        match status.as_str() {
            "completed" | "cleaned" => {
                info!(
                    target: "lmforge_cleanup",
                    status = %status,
                    "build already completed or cleaned"
                );
                Ok(RecoveryResult::AlreadyCompleted)
            }
            "in_progress" | "failed" => {
                info!(
                    target: "lmforge_cleanup",
                    status = %status,
                    "build was interrupted, cleaning up"
                );

                self.cleanup_workspace(build_path)?;

                Ok(RecoveryResult::Recovered)
            }
            _ => {
                warn!(
                    target: "lmforge_cleanup",
                    status = %status,
                    "unknown build status, treating as failed"
                );

                self.cleanup_workspace(build_path)?;

                Ok(RecoveryResult::Recovered)
            }
        }
    }

    pub fn cleanup_temp_files(&self) -> Result<()> {
        if let Some(layout) = &self.workspace_layout {
            info!(
                target: "lmforge_cleanup",
                temp_dir = ?layout.temp,
                "cleaning up temporary files"
            );

            self.workspace_manager.cleanup_temp(layout)?;

            info!(target: "lmforge_cleanup", "temporary files cleaned");
        }

        Ok(())
    }

    pub fn cleanup_workspace(&self, path: &Path) -> Result<()> {
        info!(
            target: "lmforge_cleanup",
            path = ?path,
            "cleaning up workspace"
        );

        if !path.exists() {
            debug!(target: "lmforge_cleanup", path = ?path, "workspace does not exist, nothing to clean");
            return Ok(());
        }

        self.unmount_all_mounts(path)?;
        self.remove_directory_recursive(path)?;

        info!(target: "lmforge_cleanup", path = ?path, "workspace cleaned successfully");

        Ok(())
    }

    fn unmount_all_mounts(&self, workspace_root: &Path) -> Result<()> {
        info!(target: "lmforge_cleanup", "unmounting all mounts from workspace");

        let mount_points = vec![
            workspace_root.join("rootfs/proc"),
            workspace_root.join("rootfs/sys"),
            workspace_root.join("rootfs/dev/pts"),
            workspace_root.join("rootfs/dev"),
            workspace_root.join("rootfs/run"),
            workspace_root.join("chroot/proc"),
            workspace_root.join("chroot/sys"),
            workspace_root.join("chroot/dev/pts"),
            workspace_root.join("chroot/dev"),
            workspace_root.join("chroot/run"),
        ];

        for mount_point in &mount_points {
            if mount_point.exists() {
                self.try_unmount(mount_point)?;
            }
        }

        info!(target: "lmforge_cleanup", "all mounts unmounted");
        Ok(())
    }

    fn try_unmount(&self, mount_point: &Path) -> Result<()> {
        debug!(target: "lmforge_cleanup", mount = ?mount_point, "attempting to unmount");

        let rt = tokio::runtime::Runtime::new();
        
        match rt {
            Ok(rt) => {
                use crate::runtime::mount::Mount;
                
                let result = rt.block_on(async {
                    Mount::unmount(mount_point).await
                });

                if let Err(e) = result {
                    warn!(
                        target: "lmforge_cleanup",
                        mount = ?mount_point,
                        error = %e,
                        "unmount failed, trying lazy unmount"
                    );
                    
                    let lazy_result = rt.block_on(async {
                        Executor::execute(
                            &ProcessConfig::new("umount")
                                .arg("-l")
                                .arg(mount_point)
                        ).await
                    });

                    if let Err(e2) = lazy_result {
                        debug!(
                            target: "lmforge_cleanup",
                            mount = ?mount_point,
                            error = %e2,
                            "lazy unmount also failed (may not be mounted)"
                        );
                    }
                } else {
                    debug!(target: "lmforge_cleanup", mount = ?mount_point, "unmounted successfully");
                }
            }
            Err(e) => {
                warn!(
                    target: "lmforge_cleanup",
                    error = %e,
                    "failed to create runtime for unmounting"
                );
                
                let umount_result = std::process::Command::new("umount")
                    .arg("-l")
                    .arg(mount_point)
                    .output();
                
                if let Err(e) = umount_result {
                    debug!(
                        target: "lmforge_cleanup",
                        mount = ?mount_point,
                        error = %e,
                        "fallback unmount failed"
                    );
                }
            }
        }

        Ok(())
    }

    fn remove_directory_recursive(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!(target: "lmforge_cleanup", path = ?path, "directory does not exist");
            return Ok(());
        }

        info!(target: "lmforge_cleanup", path = ?path, "removing directory");

        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 3;

        loop {
            attempts += 1;
            
            match std::fs::remove_dir_all(path) {
                Ok(_) => {
                    info!(target: "lmforge_cleanup", path = ?path, attempts = attempts, "directory removed successfully");
                    return Ok(());
                }
                Err(e) => {
                    if attempts < MAX_ATTEMPTS {
                        warn!(
                            target: "lmforge_cleanup",
                            path = ?path,
                            attempt = attempts,
                            error = %e,
                            "failed to remove directory, retrying..."
                        );
                        
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        
                        self.unmount_all_mounts(path)?;
                    } else {
                        error!(
                            target: "lmforge_cleanup",
                            path = ?path,
                            attempts = attempts,
                            error = %e,
                            "failed to remove directory after {} attempts",
                            MAX_ATTEMPTS
                        );
                        
                        return Err(anyhow::anyhow!(
                            "Failed to remove directory {:?} after {} attempts: {}",
                            path,
                            MAX_ATTEMPTS,
                            e
                        ));
                    }
                }
            }
        }
    }

    pub fn mark_completed(&self) -> Result<()> {
        if let Some(layout) = &self.workspace_layout {
            let lock_file = layout.root.join(".build.lock");

            if lock_file.exists() {
                let completed_content = format!(
                    r#"build_id: {build_id}
pid: {pid}
started_at: {timestamp}
completed_at: {completed_timestamp}
status: completed
"#,
                    build_id = self.workspace_manager.build_id(),
                    pid = std::process::id(),
                    timestamp = Utc::now().to_rfc3339(),
                    completed_timestamp = Utc::now().to_rfc3339(),
                );

                std::fs::write(&lock_file, completed_content)?;

                info!(
                    target: "lmforge_cleanup",
                    file = ?lock_file,
                    "marked build as completed"
                );
            }
        }

        Ok(())
    }

    pub fn mark_failed(&self, error_message: &str) -> Result<()> {
        if let Some(layout) = &self.workspace_layout {
            let lock_file = layout.root.join(".build.lock");

            if lock_file.exists() {
                let failed_content = format!(
                    r#"build_id: {build_id}
pid: {pid}
started_at: {timestamp}
failed_at: {failed_timestamp}
status: failed
error: {error}
"#,
                    build_id = self.workspace_manager.build_id(),
                    pid = std::process::id(),
                    timestamp = Utc::now().to_rfc3339(),
                    failed_timestamp = Utc::now().to_rfc3339(),
                    error = error_message,
                );

                std::fs::write(&lock_file, failed_content)?;

                error!(
                    target: "lmforge_cleanup",
                    file = ?lock_file,
                    error = %error_message,
                    "marked build as failed"
                );
            }
        }

        Ok(())
    }

    pub fn full_cleanup(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "performing full cleanup");

        self.cleanup_temp_files()?;

        if let Some(layout) = &self.workspace_layout {
            self.cleanup_workspace(&layout.root)?;
        }

        info!(target: "lmforge_cleanup", "full cleanup completed");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RecoveryResult {
    Recovered,
    AlreadyCompleted,
    Failed(String),
}

fn parse_build_status(lock_content: &str) -> String {
    lock_content
        .lines()
        .find(|line| line.starts_with("status:"))
        .and_then(|line| line.strip_prefix("status:"))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
