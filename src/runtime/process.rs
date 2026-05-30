use std::path::PathBuf;
use std::ffi::OsStr;
use std::process::Stdio;
use std::time::Instant;
use anyhow::{Result, bail};
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn, error};

use crate::telemetry::runtime::RuntimeLogger;
use crate::runtime::log_stream::{
    StreamDispatcher,
    ProcessOutput as LogProcessOutput,
};

#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub timeout: Option<std::time::Duration>,
    pub capture_output: bool,
    pub build_id: Option<String>,
    pub stage_name: Option<String>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        ProcessConfig {
            command: String::new(),
            args: vec![],
            working_dir: None,
            env: vec![],
            timeout: None,
            capture_output: true,
            build_id: None,
            stage_name: None,
        }
    }
}

impl ProcessConfig {
    pub fn new(command: impl Into<String>) -> Self {
        ProcessConfig {
            command: command.into(),
            ..Default::default()
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_string_lossy().to_string());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        for arg in args {
            self.args.push(arg.as_ref().to_string_lossy().to_string());
        }
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn timeout(mut self, duration: std::time::Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    pub fn with_build_id(mut self, id: impl Into<String>) -> Self {
        self.build_id = Some(id.into());
        self
    }

    pub fn with_streaming(mut self) -> Self {
        self.capture_output = false;
        self
    }

    pub fn with_stage_name(mut self, name: impl Into<String>) -> Self {
        self.stage_name = Some(name.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub enum ExitStatus {
    Success,
    Failure(i32),
    Timeout,
    NotFound,
}

pub struct Executor;

impl Executor {
    pub async fn execute(config: &ProcessConfig) -> Result<ProcessOutput> {
        let logger = RuntimeLogger::new(
            config.build_id.as_deref().unwrap_or("unknown")
        );

        let args_refs: Vec<&str> = config.args.iter().map(|s| s.as_str()).collect();
        
        logger.log_process_start(
            &config.command,
            &args_refs,
            config.working_dir.as_ref()
        );

        let start_time = Instant::now();

        let mut cmd = Command::new(&config.command);

        cmd.args(&config.args)
            .stdin(Stdio::null())
            .kill_on_drop(true);

        if config.capture_output {
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped());
        }

        if let Some(ref dir) = config.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let execution = match config.timeout {
            Some(timeout) => {
                tokio::time::timeout(timeout, cmd.output()).await
            }
            None => Ok(Ok(cmd.output().await?)),
        };

        let duration = start_time.elapsed();

        match execution {
            Ok(output) => {
                let output = output?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                let status = if output.status.success() {
                    ExitStatus::Success
                } else {
                    ExitStatus::Failure(
                        output.status.code().unwrap_or(-1)
                    )
                };

                logger.log_process_complete(
                    &config.command,
                    match status {
                        ExitStatus::Success => 0,
                        ExitStatus::Failure(code) => code,
                        ExitStatus::Timeout => -1,
                        ExitStatus::NotFound => -2,
                    },
                    duration,
                    &stdout,
                    &stderr
                );

                Ok(ProcessOutput {
                    status,
                    stdout,
                    stderr,
                })
            }
            Err(_) => {
                error!(target: "lmforge_runtime", "process timed out: {}", config.command);
                
                Ok(ProcessOutput {
                    status: ExitStatus::Timeout,
                    stdout: String::new(),
                    stderr: "Process timed out".to_string(),
                })
            }
        }
    }

    pub async fn execute_streaming(
        config: &ProcessConfig,
        dispatcher: &mut StreamDispatcher,
    ) -> Result<LogProcessOutput> {
        let mut log_output = LogProcessOutput::new(
            config.command.clone(),
            config.args.clone()
        );

        let stage_name = config.stage_name.as_deref().unwrap_or("process");
        dispatcher.update_status_with_command("starting...", &config.command);

        info!(
            target: "lmforge_process",
            command = %config.command,
            args = ?config.args,
            stage = %stage_name,
            "starting streaming process execution"
        );

        let start_time = std::time::Instant::now();

        let mut cmd_builder = Command::new(&config.command);
        cmd_builder
            .args(&config.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(ref dir) = config.working_dir {
            cmd_builder.current_dir(dir);
        }

        for (key, value) in &config.env {
            cmd_builder.env(key, value);
        }

        let mut child = match cmd_builder.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!(
                    target: "lmforge_process",
                    command = %config.command,
                    error = %e,
                    "command not found"
                );
                log_output.complete(None, false);
                return Ok(log_output);
            }
            Err(e) => {
                bail!("Failed to spawn '{}': {}", config.command, e);
            }
        };

        dispatcher.update_status("running...");

        let stdout = child.stdout.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        
        let stderr = child.stderr.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut dispatcher_stdout = dispatcher.clone();
        let mut dispatcher_stderr = dispatcher.clone();

        let stdout_handle = tokio::spawn(async move {
            let mut stdout_buffer = String::new();
            
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                stdout_buffer.push_str(&line);
                stdout_buffer.push('\n');
                
                dispatcher_stdout.dispatch_stdout(&format!("{}\n", line));
            }

            stdout_buffer
        });

        let stderr_handle = tokio::spawn(async move {
            let mut stderr_buffer = String::new();
            
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                stderr_buffer.push_str(&line);
                stderr_buffer.push('\n');
                
                dispatcher_stderr.dispatch_stderr(&format!("{}\n", line));
            }

            stderr_buffer
        });

        let execution_result = match config.timeout {
            Some(timeout) => {
                tokio::time::timeout(timeout, child.wait()).await
            }
            None => Ok(child.wait().await),
        };

        let (stdout_result, stderr_result) = tokio::try_join!(stdout_handle, stderr_handle)?;

        let stdout_data = stdout_result;
        let stderr_data = stderr_result;

        log_output.stdout = stdout_data;
        log_output.stderr = stderr_data;

        match execution_result {
            Ok(Ok(status)) => {
                let exit_code = status.code();
                log_output.complete(exit_code, false);

                if exit_code == Some(0) {
                    dispatcher.update_status("completed successfully");
                    info!(
                        target: "lmforge_process",
                        command = %config.command,
                        exit_code = ?exit_code,
                        duration_secs = start_time.elapsed().as_secs(),
                        "process completed successfully"
                    );
                } else {
                    dispatcher.update_status(&format!("failed with exit code {:?}", exit_code));
                    error!(
                        target: "lmforge_process",
                        command = %config.command,
                        exit_code = ?exit_code,
                        duration_secs = start_time.elapsed().as_secs(),
                        "process failed"
                    );
                }
            }
            Ok(Err(e)) => {
                log_output.complete(None, false);
                dispatcher.update_status(&format!("error: {}", e));
                error!(
                    target: "lmforge_process",
                    command = %config.command,
                    error = %e,
                    "error waiting for process"
                );
                return Err(anyhow::anyhow!("Failed to wait for '{}': {}", config.command, e));
            }
            Err(_) => {
                log_output.complete(None, true);
                dispatcher.update_status("TIMED OUT");
                
                error!(
                    target: "lmforge_process",
                    command = %config.command,
                    timeout_secs = config.timeout.map(|t| t.as_secs()),
                    "process timed out"
                );

                let _ = child.kill().await;
            }
        }

        dispatcher.finish(&log_output)?;

        Ok(log_output)
    }

    pub async fn execute_success(config: &ProcessConfig) -> Result<String> {
        let output = Self::execute(config).await?;
        
        match output.status {
            ExitStatus::Success => Ok(output.stdout),
            ExitStatus::Failure(code) => {
                bail!(
                    "Command '{}' failed with exit code {}: {}",
                    config.command,
                    code,
                    output.stderr
                );
            }
            ExitStatus::Timeout => {
                bail!("Command '{}' timed out", config.command);
            }
            ExitStatus::NotFound => {
                bail!("Command '{}' not found", config.command);
            }
        }
    }

    pub async fn exists(command: &str) -> bool {
        which::which(command).is_ok()
    }
}
