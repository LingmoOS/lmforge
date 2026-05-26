use std::path::PathBuf;
use std::process::Stdio;
use anyhow::{Result, bail};
use tokio::process::Command;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub timeout: Option<std::time::Duration>,
    pub capture_output: bool,
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

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for arg in args {
            self.args.push(arg.into());
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
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExitStatus {
    Success,
    Failure(i32),
    Timeout,
    NotFound,
}

pub struct Executor;

impl Executor {
    pub async fn execute(config: &ProcessConfig) -> Result<ProcessOutput> {
        debug!(
            "Executing: {} {}",
            config.command,
            config.args.join(" ")
        );

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

        for (key, value) in &config.config.env {
            cmd.env(key, value);
        }

        let execution = match config.timeout {
            Some(timeout) => {
                tokio::time::timeout(timeout, cmd.output()).await
            }
            None => Ok(cmd.output().await?),
        };

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

                if !stdout.is_empty() {
                    debug!("stdout: {}", stdout);
                }
                if !stderr.is_empty() {
                    debug!("stderr: {}", stderr);
                }

                Ok(ProcessOutput {
                    status,
                    stdout,
                    stderr,
                })
            }
            Err(_) => {
                info!("Process timed out: {}", config.command);
                Ok(ProcessOutput {
                    status: ExitStatus::Timeout,
                    stdout: String::new(),
                    stderr: "Process timed out".to_string(),
                })
            }
        }
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
