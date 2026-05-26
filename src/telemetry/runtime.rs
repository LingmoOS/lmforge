use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, debug, warn, error, span, Level};

pub struct RuntimeLogger {
    build_id: String,
}

impl RuntimeLogger {
    pub fn new(build_id: impl Into<String>) -> Self {
        RuntimeLogger {
            build_id: build_id.into(),
        }
    }

    pub fn log_process_start(&self, command: &str, args: &[&str], working_dir: Option<&PathBuf>) {
        let _span = span!(Level::INFO, "process_exec", 
            command = command,
            args_count = args.len(),
            build_id = %self.build_id
        ).entered();

        info!(
            command = command,
            args = ?args,
            working_dir = ?working_dir,
            "exec: {} {}",
            command,
            args.join(" ")
        );
    }

    pub fn log_process_complete(
        &self,
        command: &str,
        exit_code: i32,
        duration: std::time::Duration,
        stdout: &str,
        stderr: &str,
    ) {
        let _span = span!(Level::DEBUG, "process_exit",
            command = command,
            exit_code = exit_code,
            duration_ms = duration.as_millis() as u64,
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            build_id = %self.build_id
        ).entered();

        info!(
            command = command,
            exit_code = exit_code,
            duration_ms = duration.as_millis() as u64,
            "process completed: {} (exit={})",
            command,
            exit_code
        );

        if !stdout.is_empty() {
            debug!(stdout = %stdout, "stdout:");
        }

        if !stderr.is_empty() {
            if exit_code == 0 {
                debug!(stderr = %stderr, "stderr (warnings):");
            } else {
                error!(stderr = %stderr, "stderr (errors):");
            }
        }
    }

    pub fn log_process_error(&self, command: &str, error: &dyn std::error::Error) {
        error!(
            command = command,
            error = %error,
            build_id = %self.build_id,
            "process execution failed: {}",
            error
        );
    }

    pub fn log_mount(&self, source: &PathBuf, target: &PathBuf, fs_type: &str) {
        let _span = span!(Level::INFO, "mount_operation",
            source = %source.display(),
            target = %target.display(),
            fs_type = fs_type,
            build_id = %self.build_id
        ).entered();

        info!(
            source = %source.display(),
            target = %target.display(),
            fs_type = fs_type,
            "mount: {} -> {} ({})",
            source.display(),
            target.display(),
            fs_type
        );
    }

    pub fn log_unmount(&self, target: &PathBuf) {
        let _span = span!(Level::INFO, "unmount_operation",
            target = %target.display(),
            build_id = %self.build_id
        ).entered();

        info!(
            target = %target.display(),
            "unmount: {}",
            target.display()
        );
    }

    pub fn log_workspace_create(&self, path: &PathBuf) {
        let _span = span!(Level::INFO, "workspace_lifecycle",
            action = "create",
            path = %path.display(),
            build_id = %self.build_id
        ).entered();

        info!(
            path = %path.display(),
            "workspace create: {}",
            path.display()
        );
    }

    pub fn log_workspace_cleanup(&self, path: &PathBuf) {
        let _span = span!(Level::INFO, "workspace_lifecycle",
            action = "cleanup",
            path = %path.display(),
            build_id = %self.build_id
        ).entered();

        info!(
            path = %path.display(),
            "workspace cleanup: {}",
            path.display()
        );
    }

    pub fn log_sandbox_enter(&self, root: &PathBuf) {
        let _span = span!(Level::INFO, "sandbox_lifecycle",
            action = "enter",
            root = %root.display(),
            build_id = %self.build_id
        ).entered();

        info!(
            root = %root.display(),
            "sandbox enter: {}",
            root.display()
        );
    }

    pub fn log_sandbox_exit(&self, root: &PathBuf) {
        let _span = span!(Level::INFO, "sandbox_lifecycle",
            action = "exit",
            root = %root.display(),
            build_id = %self.build_id
        ).entered();

        info!(
            root = %root.display(),
            "sandbox exit: {}",
            root.display()
        );
    }

    pub fn log_stage_start(&self, stage: &str) {
        let _span = span!(Level::INFO, "stage_execution",
            stage = stage,
            action = "start",
            build_id = %self.build_id
        ).entered();

        info!(stage = stage, "[{}] starting", stage);
    }

    pub fn log_stage_complete(&self, stage: &str, duration: std::time::Duration) {
        let _span = span!(Level::INFO, "stage_execution",
            stage = stage,
            action = "complete",
            duration_ms = duration.as_millis() as u64,
            build_id = %self.build_id
        ).entered();

        info!(
            stage = stage,
            duration_ms = duration.as_millis() as u64,
            "[{}] complete ({:.2}s)",
            stage,
            duration.as_secs_f64()
        );
    }

    pub fn log_stage_error(&self, stage: &str, error: &str) {
        let _span = span!(Level::ERROR, "stage_execution",
            stage = stage,
            action = "error",
            error = error,
            build_id = %self.build_id
        ).entered();

        error!(stage = stage, error = error, "[{}] failed: {}", stage, error);
    }
}
