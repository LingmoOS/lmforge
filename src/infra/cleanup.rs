use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};
use chrono::Utc;

use crate::infra::workspace::{WorkspaceManager, WorkspaceLayout, InterruptedBuild};
use crate::runtime::{
    mount_manager::MountManager,
};

#[derive(Clone)]
pub struct CleanupRecovery {
    workspace_manager: WorkspaceManager,
    workspace_layout: Option<WorkspaceLayout>,
    mount_manager: Option<Arc<MountManager>>,
    cleanup_completed: Arc<std::sync::atomic::AtomicBool>,
}

impl CleanupRecovery {
    pub fn new(workspace_manager: WorkspaceManager) -> Self {
        CleanupRecovery {
            workspace_manager,
            workspace_layout: None,
            mount_manager: None,
            cleanup_completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn with_workspace(mut self, layout: WorkspaceLayout) -> Self {
        self.workspace_layout = Some(layout);
        self
    }

    pub fn with_mount_manager(mut self, manager: Arc<MountManager>) -> Self {
        self.mount_manager = Some(manager);
        self
    }

    pub fn clone_without_workspace(&self) -> Self {
        CleanupRecovery {
            workspace_manager: WorkspaceManager::new(
                self.workspace_layout.as_ref()
                    .map(|l| l.root.parent().unwrap_or(Path::new(".")).to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("./workspace")),
                &self.workspace_manager.build_id()
            ),
            workspace_layout: None,
            mount_manager: self.mount_manager.clone(),
            cleanup_completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn initialize(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "initializing cleanup and recovery system");

        if self.cleanup_completed.load(std::sync::atomic::Ordering::SeqCst) {
            debug!(target: "lmforge_cleanup", "cleanup already completed, skipping initialization");
            return Ok(());
        }

        self.cleanup_stale_workspaces()?;
        self.detect_and_recover_stale_mounts()?;
        self.detect_interrupted_builds()?;

        if let Some(layout) = &self.workspace_layout {
            self.create_lock_file(layout)?;
            self.create_pid_file(layout)?;
        }

        info!(target: "lmforge_cleanup", "cleanup and recovery system initialized (idempotent)");
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

    pub fn cleanup_stale_workspaces(&self) -> Result<Vec<PathBuf>> {
        info!(target: "lmforge_cleanup", "checking for stale workspaces (idempotent)");

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

        Ok(cleaned)
    }

    pub fn detect_interrupted_builds(&self) -> Result<Vec<InterruptedBuild>> {
        info!(target: "lmforge_cleanup", "scanning for interrupted builds (idempotent)");

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

                self.recover_build(&build.path)?;
            }
        } else {
            debug!(target: "lmforge_cleanup", "no interrupted builds found");
        }

        Ok(interrupted)
    }

    fn detect_and_recover_stale_mounts(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "checking for stale mounts from previous runs");

        if let Some(base_dir) = self.get_base_dir() {
            self.recover_mounts_in_directory(&base_dir)?;
        }

        Ok(())
    }

    fn recover_mounts_in_directory(&self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        let known_mount_points = [
            "rootfs/proc",
            "rootfs/sys",
            "rootfs/dev/pts",
            "rootfs/dev",
            "rootfs/run",
            "chroot/proc",
            "chroot/sys",
            "chroot/dev/pts",
            "chroot/dev",
            "chroot/run",
        ];

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("build-"))
                .unwrap_or(false)
            {
                for mount_rel in &known_mount_points {
                    let mount_path = path.join(mount_rel);
                    
                    if self.is_mounted(&mount_path) {
                        warn!(
                            target: "lmforge_cleanup",
                            mount = ?mount_path,
                            "stale mount detected, recovering"
                        );
                        
                        self.safe_unmount(&mount_path)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn is_mounted(&self, path: &Path) -> bool {
        if !path.exists() {
            return false;
        }

        match std::process::Command::new("mountpoint")
            .arg("-q")
            .arg(path)
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn safe_unmount(&self, path: &Path) -> Result<()> {
        debug!(
            target: "lmforge_cleanup",
            mount = ?path,
            "attempting safe unmount (idempotent)"
        );

        if !path.exists() {
            debug!(
                target: "lmforge_cleanup",
                mount = ?path,
                "path does not exist, skipping unmount (idempotent)"
            );
            return Ok(());
        }

        if !self.is_mounted(path) {
            debug!(
                target: "lmforge_cleanup",
                mount = ?path,
                "not mounted, skipping (idempotent)"
            );
            return Ok(());
        }

        let result = std::process::Command::new("umount")
            .arg("-l")  
            .arg(path)
            .output();

        match result {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                if output.status.success() || 
                   stderr.contains("not mounted") ||
                   stderr.contains("not found") ||
                   stderr.contains("No such file or directory") {
                    info!(
                        target: "lmforge_cleanup",
                        mount = ?path,
                        exit_code = output.status.code().unwrap_or(-1),
                        "unmount completed (idempotent)"
                    );
                    Ok(())
                } else if output.status.code() == Some(32) {
                    debug!(
                        target: "lmforge_cleanup",
                        mount = ?path,
                        "exit code 32 - not mounted, treating as success (idempotent)"
                    );
                    Ok(())
                } else {
                    warn!(
                        target: "lmforge_cleanup",
                        mount = ?path,
                        exit_code = output.status.code(),
                        stderr = %stderr,
                        "unmount failed with unexpected error"
                    );
                    
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    let retry_result = std::process::Command::new("umount")
                        .arg("-lf")
                        .arg(path)
                        .output();
                        
                    match retry_result {
                        Ok(retry_output) => {
                            let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);
                            if retry_output.status.success() ||
                               retry_stderr.contains("not mounted") {
                                info!(
                                    target: "lmforge_cleanup",
                                    mount = ?path,
                                    "force lazy unmount succeeded on retry"
                                );
                                Ok(())
                            } else {
                                warn!(
                                    target: "lmforge_cleanup",
                                    mount = ?path,
                                    stderr = %retry_stderr,
                                    "force lazy unmount also failed, continuing cleanup..."
                                );
                                Ok(())
                            }
                        }
                        Err(e) => {
                            warn!(
                                target: "lmforge_cleanup",
                                mount = ?path,
                                error = %e,
                                "unmount command failed, continuing cleanup..."
                            );
                            Ok(())
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    target: "lmforge_cleanup",
                    mount = ?path,
                    error = %e,
                    "error executing umount command, continuing cleanup..."
                );
                Ok(())
            }
        }
    }

    pub fn recover_interrupted_build(&self, build_path: &Path) -> Result<RecoveryResult> {
        info!(
            target: "lmforge_cleanup",
            path = ?build_path,
            "recovering interrupted build (idempotent)"
        );

        let lock_file = build_path.join(".build.lock");
        
        if !lock_file.exists() {
            debug!(target: "lmforge_cleanup", path = ?build_path, "no lock file, skipping recovery");
            return Ok(RecoveryResult::AlreadyCompleted);
        }

        let lock_content = std::fs::read_to_string(&lock_file)?;
        let status = parse_build_status(&lock_content);

        match status.as_str() {
            "completed" | "cleaned" => {
                debug!(
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
                debug!(
                    target: "lmforge_cleanup",
                    status = %status,
                    "unknown build status, treating as failed"
                );

                self.cleanup_workspace(build_path)?;

                Ok(RecoveryResult::Recovered)
            }
        }
    }

    fn recover_build(&self, build_path: &Path) -> Result<()> {
        info!(
            target: "lmforge_cleanup",
            path = ?build_path,
            "recovering interrupted build"
        );

        self.unmount_all_from_workspace(build_path)?;
        
        if let Err(e) = self.remove_directory_with_retry(build_path) {
            warn!(
                target: "lmforge_cleanup",
                path = ?build_path,
                error = %e,
                "could not fully clean up interrupted build"
            );
        }

        Ok(())
    }

    pub fn cleanup_temp_files(&self) -> Result<()> {
        if self.cleanup_completed.load(std::sync::atomic::Ordering::SeqCst) {
            debug!(target: "lmforge_cleanup", "temp files already cleaned up (idempotent)");
            return Ok(());
        }

        if let Some(layout) = &self.workspace_layout {
            info!(
                target: "lmforge_cleanup",
                temp_dir = ?layout.temp,
                "cleaning up temporary files (idempotent)"
            );

            if layout.temp.exists() {
                self.workspace_manager.cleanup_temp(layout)?;
            }

            info!(target: "lmforge_cleanup", "temporary files cleaned");
        }

        Ok(())
    }

    pub fn cleanup_workspace(&self, path: &Path) -> Result<()> {
        info!(
            target: "lmforge_cleanup",
            path = ?path,
            "cleaning up workspace (idempotent)"
        );

        if !path.exists() {
            debug!(target: "lmforge_cleanup", path = ?path, "workspace does not exist, nothing to clean");
            return Ok(());
        }

        self.unmount_all_from_workspace(path)?;
        self.remove_directory_with_retry(path)?;

        info!(target: "lmforge_cleanup", path = ?path, "workspace cleaned successfully (idempotent)");

        Ok(())
    }

    fn unmount_all_from_workspace(&self, workspace_root: &Path) -> Result<()> {
        info!(target: "lmforge_cleanup", root = ?workspace_root, "unmounting all mounts in reverse order");

        if let Some(ref mm) = self.mount_manager {
            mm.force_cleanup_all()?;
        } else {
            self.unmount_known_mounts(workspace_root)?;
        }

        info!(target: "lmforge_cleanup", root = ?workspace_root, "all mounts unmounted");
        Ok(())
    }

    fn unmount_known_mounts(&self, workspace_root: &Path) -> Result<()> {
        let mount_points = vec![
            workspace_root.join("rootfs/run"),
            workspace_root.join("rootfs/dev/pts"),
            workspace_root.join("rootfs/dev"),
            workspace_root.join("rootfs/sys"),
            workspace_root.join("rootfs/proc"),
            workspace_root.join("chroot/run"),
            workspace_root.join("chroot/dev/pts"),
            workspace_root.join("chroot/dev"),
            workspace_root.join("chroot/sys"),
            workspace_root.join("chroot/proc"),
        ];

        for mount_point in &mount_points {
            debug!(
                target: "lmforge_cleanup",
                mount = ?mount_point,
                "attempting to unmount known mount point (idempotent)"
            );
            
            if let Err(e) = self.safe_unmount(mount_point) {
                debug!(
                    target: "lmforge_cleanup",
                    mount = ?mount_point,
                    error = %e,
                    "unmount failed (may not be mounted), continuing..."
                );
            }
        }

        Ok(())
    }

    fn remove_directory_with_retry(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            debug!(target: "lmforge_cleanup", path = ?path, "directory does not exist (idempotent)");
            return Ok(());
        }

        info!(target: "lmforge_cleanup", path = ?path, "removing directory with aggressive retry (idempotent)");

        const MAX_ATTEMPTS: u32 = 5;
        let retry_delays: [u64; 5] = [200, 500, 1000, 2000, 3000];

        for attempt in 1..=MAX_ATTEMPTS {
            debug!(
                target: "lmforge_cleanup",
                path = ?path,
                attempt = attempt,
                max_attempts = MAX_ATTEMPTS,
                "attempt {} of {} to remove directory",
                attempt, MAX_ATTEMPTS
            );

            self.aggressive_unmount_all(path)?;

            match std::fs::remove_dir_all(path) {
                Ok(_) => {
                    info!(
                        target: "lmforge_cleanup",
                        path = ?path,
                        attempt = attempt,
                        "directory removed successfully after {} attempt(s) (idempotent)",
                        attempt
                    );
                    return Ok(());
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let is_permission_error = error_msg.contains("Operation not permitted") ||
                                            error_msg.contains("Permission denied") ||
                                            error_msg.contains("Device or resource busy");
                    
                    if attempt < MAX_ATTEMPTS {
                        warn!(
                            target: "lmforge_cleanup",
                            path = ?path,
                            attempt = attempt,
                            max_attempts = MAX_ATTEMPTS,
                            error = %e,
                            is_permission_error = is_permission_error,
                            "failed to remove directory, waiting {}ms before retry...",
                            retry_delays[attempt as usize - 1]
                        );
                        
                        std::thread::sleep(std::time::Duration::from_millis(retry_delays[attempt as usize - 1]));
                        
                        self.aggressive_unmount_all(path)?;
                    } else {
                        warn!(
                            target: "lmforge_cleanup",
                            path = ?path,
                            attempts = MAX_ATTEMPTS,
                            error = %e,
                            "std::fs::remove_dir_all failed after {} attempts, trying system rm -rf...",
                            MAX_ATTEMPTS
                        );

                        let rm_result = std::process::Command::new("rm")
                            .arg("-rf")
                            .arg(path)
                            .output();

                        match rm_result {
                            Ok(output) => {
                                if output.status.success() || !path.exists() {
                                    info!(
                                        target: "lmforge_cleanup",
                                        path = ?path,
                                        "system rm -rf succeeded (idempotent)"
                                    );
                                    return Ok(());
                                } else {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    error!(
                                        target: "lmforge_cleanup",
                                        path = ?path,
                                        exit_code = output.status.code(),
                                        stderr = %stderr,
                                        "CRITICAL: Failed to remove directory even with rm -rf"
                                    );
                                    
                                    if !path.exists() {
                                        info!(
                                            target: "lmforge_cleanup",
                                            path = ?path,
                                            "directory no longer exists despite error (idempotent)"
                                        );
                                        return Ok(());
                                    }
                                    
                                    return Err(anyhow::anyhow!(
                                        "CRITICAL: Failed to remove directory {:?} after all attempts.\n\
                                         Last error: {}\n\
                                         rm -rf stderr: {}",
                                        path,
                                        e,
                                        stderr
                                    ));
                                }
                            }
                            Err(rm_err) => {
                                error!(
                                    target: "lmforge_cleanup",
                                    path = ?path,
                                    error = %rm_err,
                                    original_error = %e,
                                    "CRITICAL: Both remove_dir_all and rm -rf failed"
                                );
                                
                                if !path.exists() {
                                    info!(
                                        target: "lmforge_cleanup",
                                        path = ?path,
                                        "directory no longer exists despite errors (idempotent)"
                                    );
                                    return Ok(());
                                }

                                return Err(anyhow::anyhow!(
                                    "CRITICAL: All removal methods failed for {:?}:\n\
                                     - remove_dir_all: {}\n\
                                     - rm -rf: {}",
                                    path,
                                    e,
                                    rm_err
                                ));
                            }
                        }
                    }
                }
            }
        }

        unreachable!()
    }

    fn aggressive_unmount_all(&self, workspace_root: &Path) -> Result<()> {
        debug!(target: "lmforge_cleanup", root = ?workspace_root, "performing aggressive unmount");

        self.unmount_known_mounts(workspace_root)?;
        
        self.scan_and_unmount_from_proc_mounts(workspace_root)?;
        
        self.unmount_subdirectories_recursive(workspace_root)?;

        Ok(())
    }

    fn scan_and_unmount_from_proc_mounts(&self, workspace_root: &Path) -> Result<()> {
        debug!(target: "lmforge_cleanup", root = ?workspace_root, "scanning /proc/mounts for mounts in workspace");

        if let Ok(mounts_content) = std::fs::read_to_string("/proc/mounts") {
            let workspace_str = workspace_root.to_string_lossy().to_string();
            
            for line in mounts_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                
                if parts.len() >= 2 {
                    let mount_point = parts[1];
                    
                    if mount_point.starts_with(&workspace_str) || 
                       mount_point.contains("build-") {
                        let mount_path = PathBuf::from(mount_point);
                        
                        if self.is_mounted(&mount_path) {
                            debug!(
                                target: "lmforge_cleanup",
                                mount = ?mount_path,
                                "found mount in /proc/mounts, unmounting"
                            );
                            
                            if let Err(e) = self.safe_unmount(&mount_path) {
                                debug!(
                                    target: "lmforge_cleanup",
                                    mount = ?mount_path,
                                    error = %e,
                                    "failed to unmount from /proc/mounts scan"
                                );
                            }
                        }
                    }
                }
            }
        } else {
            debug!(target: "lmforge_cleanup", "could not read /proc/mounts");
        }

        Ok(())
    }

    fn unmount_subdirectories_recursive(&self, root: &Path) -> Result<()> {
        if !root.exists() {
            return Ok(());
        }

        let common_mount_points = ["proc", "sys", "dev", "run", "tmp", "boot"];

        if let Ok(entries) = std::fs::read_dir(root) {
            let mut subdirs = Vec::new();
            
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    subdirs.push(entry.path());
                }
            }

            for subdir in &subdirs {
                for mount_name in &common_mount_points {
                    let mount_path = subdir.join(mount_name);
                    
                    if self.is_mounted(&mount_path) {
                        debug!(
                            target: "lmforge_cleanup",
                            mount = ?mount_path,
                            "found potential mount point in subdirectory"
                        );
                        
                        if let Err(e) = self.safe_unmount(&mount_path) {
                            debug!(
                                target: "lmforge_cleanup",
                                mount = ?mount_path,
                                error = %e,
                                "failed to unmount subdirectory mount"
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn mark_completed(&self) -> Result<()> {
        if self.cleanup_completed.load(std::sync::atomic::Ordering::SeqCst) {
            debug!(target: "lmforge_cleanup", "already marked as completed (idempotent)");
            return Ok(());
        }

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
                    "marked build as completed (idempotent)"
                );
            }
        }

        self.cleanup_completed.store(true, std::sync::atomic::Ordering::SeqCst);

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
                    "marked build as failed (idempotent)"
                );
            }
        }

        Ok(())
    }

    pub fn full_cleanup(&self) -> Result<()> {
        info!(target: "lmforge_cleanup", "performing full cleanup (idempotent)");

        if self.cleanup_completed.load(std::sync::atomic::Ordering::SeqCst) {
            debug!(target: "lmforge_cleanup", "full cleanup already completed (idempotent)");
            return Ok(());
        }

        self.cleanup_temp_files()?;

        if let Some(layout) = &self.workspace_layout {
            self.cleanup_workspace(&layout.root)?;
            self.cleanup_livebuild_residuals(layout)?;
        }

        self.cleanup_completed.store(true, std::sync::atomic::Ordering::SeqCst);

        info!(target: "lmforge_cleanup", "full cleanup completed (idempotent)");
        Ok(())
    }

    pub fn cleanup_livebuild_residuals(&self, layout: &WorkspaceLayout) -> Result<()> {
        info!(
            target: "lmforge_cleanup",
            stage = "livebuild",
            "cleaning up live-build residuals for V1 architecture"
        );

        let lb_config_dir = layout.livebuild_config();
        
        if lb_config_dir.exists() {
            info!(
                target: "lmforge_cleanup",
                lb_config = ?lb_config_dir,
                "found live-build configuration directory, cleaning up"
            );

            let clean_result = std::process::Command::new("lb")
                .arg("clean")
                .arg("--all")
                .current_dir(&lb_config_dir)
                .output();

            match clean_result {
                Ok(output) => {
                    if output.status.success() {
                        info!(
                            target: "lmforge_cleanup",
                            lb_config = ?lb_config_dir,
                            "lb clean --all completed successfully"
                        );
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        warn!(
                            target: "lmforge_cleanup",
                            lb_config = ?lb_config_dir,
                            exit_code = output.status.code(),
                            stderr = %stderr,
                            "lb clean --all failed, will remove directory manually"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        target: "lmforge_cleanup",
                        lb_config = ?lb_config_dir,
                        error = %e,
                        "failed to execute lb clean, removing directory manually"
                    );
                }
            }

            if let Err(e) = self.remove_directory_with_retry(&lb_config_dir) {
                warn!(
                    target: "lmforge_cleanup",
                    lb_config = ?lb_config_dir,
                    error = %e,
                    "failed to remove live-build config directory"
                );
            } else {
                debug!(
                    target: "lmforge_cleanup",
                    lb_config = ?lb_config_dir,
                    "live-build configuration directory removed"
                );
            }
        }

        let lb_cache_dirs = [
            layout.cache.join("live-build"),
            layout.cache.join("apt"),
            layout.root.join(".build"),
        ];

        for cache_dir in &lb_cache_dirs {
            if cache_dir.exists() && cache_dir.starts_with(&layout.root) {
                debug!(
                    target: "lmforge_cleanup",
                    cache_dir = ?cache_dir,
                    "cleaning live-build cache directory"
                );
                
                if let Err(e) = self.remove_directory_with_retry(cache_dir) {
                    debug!(
                        target: "lmforge_cleanup",
                        cache_dir = ?cache_dir,
                        error = %e,
                        "failed to remove cache directory (non-critical)"
                    );
                }
            }
        }

        info!(
            target: "lmforge_cleanup",
            stage = "livebuild",
            "live-build residuals cleaned up for V1 architecture"
        );

        Ok(())
    }

    fn get_base_dir(&self) -> Option<PathBuf> {
        self.workspace_layout.as_ref()
            .map(|l| l.root.parent().map(|p| p.to_path_buf()))
            .flatten()
            .or_else(|| {
                Some(PathBuf::from("./workspace"))
            })
    }

    pub fn verify_no_mounts_remaining(&self) -> Result<bool> {
        if let Some(layout) = &self.workspace_layout {
            let known_mounts = [
                layout.root.join("rootfs/proc"),
                layout.root.join("rootfs/sys"),
                layout.root.join("rootfs/dev/pts"),
                layout.root.join("rootfs/dev"),
                layout.root.join("rootfs/run"),
            ];

            let mut remaining = Vec::new();

            for mount in &known_mounts {
                if self.is_mounted(mount) {
                    remaining.push(mount.clone());
                }
            }

            if remaining.is_empty() {
                info!(target: "lmforge_cleanup", "verification passed: no mounts remaining");
                Ok(true)
            } else {
                error!(
                    target: "lmforge_cleanup",
                    remaining = ?remaining,
                    count = remaining.len(),
                    "VERIFICATION FAILED: {} mounts still remaining!",
                    remaining.len()
                );
                Ok(false)
            }
        } else {
            Ok(true)
        }
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
