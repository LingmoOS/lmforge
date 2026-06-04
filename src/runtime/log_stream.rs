use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::io::Write;
use std::time::{Instant, Duration};
use anyhow::{Result, Context};
use chrono::Utc;
use tracing::{info, warn};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Silent = 0,
    Normal = 1,
    Verbose = 2,
    Detailed = 3,
    Trace = 4,
    Debug = 5,
}

impl LogLevel {
    pub fn from_int(level: u8) -> Self {
        match level {
            0 => LogLevel::Silent,
            1 => LogLevel::Normal,
            2 => LogLevel::Verbose,
            3 => LogLevel::Detailed,
            4 => LogLevel::Trace,
            _ => LogLevel::Debug,
        }
    }

    pub fn as_int(&self) -> u8 {
        match self {
            LogLevel::Silent => 0,
            LogLevel::Normal => 1,
            LogLevel::Verbose => 2,
            LogLevel::Detailed => 3,
            LogLevel::Trace => 4,
            LogLevel::Debug => 5,
        }
    }

    pub fn is_silent(&self) -> bool {
        *self == LogLevel::Silent
    }

    pub fn should_show_status(&self) -> bool {
        self.as_int() >= 1
    }

    pub fn should_show_substage(&self) -> bool {
        self.as_int() >= 2
    }

    pub fn should_show_key_logs(&self) -> bool {
        self.as_int() >= 3
    }

    pub fn should_show_all_logs(&self) -> bool {
        self.as_int() >= 4
    }

    pub fn should_passthrough(&self) -> bool {
        *self == LogLevel::Debug
    }

    pub fn status_update_interval(&self) -> Duration {
        match self {
            LogLevel::Silent => Duration::from_secs(u64::MAX),
            LogLevel::Normal => Duration::from_secs(60),
            LogLevel::Verbose => Duration::from_secs(60),
            LogLevel::Detailed => Duration::from_secs(30),
            LogLevel::Trace | LogLevel::Debug => Duration::from_secs(10),
        }
    }

    pub fn heartbeat_interval(&self) -> Duration {
        match self {
            LogLevel::Silent => Duration::from_secs(u64::MAX),
            _ => Duration::from_secs(60),
        }
    }

    fn is_noise_line(&self, line: &str) -> bool {
        if self.as_int() < 4 {
            let noise_patterns = [
                "Get:",
                "Hit:",
                "Fetched:",
                "Reading package lists...",
                "Building dependency tree...",
                "Reading state information...",
                "Progress: [",
                "%]",
                "Selecting previously unselected package",
                "Setting up ",
                "Preparing to unpack",
                "Unpacking ",
                "dpkg: warning:",
            ];

            noise_patterns.iter().any(|pattern| line.starts_with(pattern) || line.contains(pattern))
        } else if *self == LogLevel::Trace {
            let progress_patterns = [
                "Progress: [",
            ];
            progress_patterns.iter().any(|p| line.contains(p))
        } else {
            false
        }
    }

    fn is_key_log_line(&self, line: &str) -> bool {
        let key_prefixes = [
            "I: ", 
            "P: ",
            "E: ",
            "W: ",
            "lb: ",
            "Error:",
            "error:",
            "Warning:",
            "warning:",
            "Configuring ",
            "Installing ",
            "Removing ",
            "Setting up linux-",
            "Generating locale",
            "Updating initramfs",
            "Creating filesystem",
            "Building ISO",
            "Extracting ",
        ];

        key_prefixes.iter().any(|prefix| line.starts_with(prefix))
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Debug
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Silent => write!(f, "silent"),
            LogLevel::Normal => write!(f, "normal"),
            LogLevel::Verbose => write!(f, "verbose"),
            LogLevel::Detailed => write!(f, "detailed"),
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "silent" | "0" => Ok(LogLevel::Silent),
            "normal" | "1" => Ok(LogLevel::Normal),
            "verbose" | "2" => Ok(LogLevel::Verbose),
            "detailed" | "3" => Ok(LogLevel::Detailed),
            "trace" | "4" => Ok(LogLevel::Trace),
            "debug" | "5" => Ok(LogLevel::Debug),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub timed_out: bool,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<Duration>,
    pub command: String,
    pub args: Vec<String>,
}

impl ProcessOutput {
    pub fn new(command: String, args: Vec<String>) -> Self {
        ProcessOutput {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            success: false,
            timed_out: false,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            command,
            args,
        }
    }

    pub fn complete(&mut self, exit_code: Option<i32>, timed_out: bool) {
        self.exit_code = exit_code;
        self.end_time = Some(Utc::now());
        self.duration = self.end_time.map(|end| {
            (end - self.start_time).to_std().unwrap_or(Duration::ZERO)
        });
        self.timed_out = timed_out;
        self.success = !timed_out && exit_code.map_or(false, |code| code == 0);
    }

    pub fn get_last_lines(&self, count: usize) -> Vec<String> {
        let combined = format!("{}\n{}", self.stdout, self.stderr);
        let lines: Vec<&str> = combined.lines().collect();
        let start_idx = if lines.len() > count { lines.len() - count } else { 0 };
        lines[start_idx..].iter().map(|s| s.to_string()).collect::<Vec<_>>()
    }
}

pub struct StageStatus {
    name: String,
    status: String,
    substage: Option<String>,
    start_time: Instant,
    last_status_update: Instant,
    last_output_time: Instant,
    line_count: u64,
}

impl StageStatus {
    pub fn new(name: &str) -> Self {
        let now = Instant::now();
        StageStatus {
            name: name.to_string(),
            status: "initializing".to_string(),
            substage: None,
            start_time: now,
            last_status_update: now,
            last_output_time: now,
            line_count: 0,
        }
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = status.to_string();
        self.last_status_update = Instant::now();
        self.last_output_time = Instant::now();
    }

    pub fn set_substage(&mut self, substage: &str) {
        self.substage = Some(substage.to_string());
    }

    pub fn increment_line(&mut self) {
        self.line_count += 1;
        self.last_output_time = Instant::now();
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn since_last_update(&self) -> Duration {
        self.last_status_update.elapsed()
    }

    pub fn since_last_output(&self) -> Duration {
        self.last_output_time.elapsed()
    }

    pub fn format_elapsed(&self) -> String {
        let duration = self.elapsed();
        let total_secs = duration.as_secs();
        
        if total_secs >= 3600 {
            format!("{:02}:{:02}:{:02}", 
                total_secs / 3600, 
                (total_secs % 3600) / 60, 
                total_secs % 60)
        } else {
            format!("{:02}:{:02}", 
                total_secs / 60, 
                total_secs % 60)
        }
    }

    pub fn display(&self, show_substage: bool) -> String {
        let base = format!("[{}]: {} ({})", 
            self.name.to_uppercase(), 
            self.status, 
            self.format_elapsed());

        if show_substage {
            if let Some(ref sub) = self.substage {
                format!("{} > {}", base, sub)
            } else {
                base
            }
        } else {
            base
        }
    }
}

impl Clone for StageStatus {
    fn clone(&self) -> Self {
        StageStatus {
            name: self.name.clone(),
            status: self.status.clone(),
            substage: self.substage.clone(),
            start_time: self.start_time,
            last_status_update: self.last_status_update,
            last_output_time: self.last_output_time,
            line_count: self.line_count,
        }
    }
}

fn print_to_console(level: &LogLevel, message: &str) {
    if level.should_show_status() || level.should_passthrough() {
        println!("{}", message);
    }
}

fn print_stage_status(status: &StageStatus, level: &LogLevel) {
    if level.is_silent() {
        return;
    }

    let show_substage = level.should_show_substage();
    print_to_console(level, &status.display(show_substage));
}

fn print_stage_complete(status: &StageStatus, success: bool, level: &LogLevel) {
    if level.is_silent() {
        if success {
            println!("[DONE]: Success");
        } else {
            println!("[FAIL]: Failed");
        }
        return;
    }

    if level.should_passthrough() {
        return;
    }

    if success {
        println!("[✓] {} completed in {}", status.name.to_uppercase(), status.format_elapsed());
    } else {
        println!("[✗] {} failed after {}", status.name.to_uppercase(), status.format_elapsed());
    }
}

fn print_error_summary(stage: &str, output: &ProcessOutput, log_path: &Path, level: &LogLevel) {
    println!("\n[*Err]({}):", stage.to_uppercase());

    if output.timed_out {
        println!("  Process timed out after {:?}", output.duration);
    } else {
        println!("  '{}' exited with code {:?}", output.command, output.exit_code);
    }

    println!("\nLast output:");
    println!("{}", "─".repeat(50));

    let last_lines = output.get_last_lines(15);
    for line in &last_lines {
        println!("  {}", line);
    }

    println!("{}", "─".repeat(50));
    
    println!("\nSee: {:?}", log_path);

    if level.as_int() < 4 {
        println!("Tip: Use --log-level 4 or higher for full live-build output");
    }
}

pub struct LogFileWriter {
    log_file: PathBuf,
    file: Option<std::fs::File>,
    stage_name: String,
    build_id: String,
}

impl LogFileWriter {
    pub fn new(log_dir: &Path, stage_name: &str) -> Result<Self> {
        std::fs::create_dir_all(log_dir)
            .with_context(|| format!("Failed to create log directory: {:?}", log_dir))?;

        let stages_dir = log_dir.join("stages");
        std::fs::create_dir_all(&stages_dir)?;

        let filename = format!("{}.log", stage_name);
        let log_file = stages_dir.join(filename);

        let file = std::fs::File::create(&log_file)
            .with_context(|| format!("Failed to create log file: {:?}", log_file))?;

        info!(
            target: "lmforge_logging",
            file = ?log_file,
            stage = %stage_name,
            "opened stage log file for writing"
        );

        Ok(LogFileWriter {
            log_file,
            file: Some(file),
            stage_name: stage_name.to_string(),
            build_id: Utc::now().format("%Y%m%d-%H%M%S").to_string(),
        })
    }

    pub fn write_header(&mut self, command: &str, args: &[String]) -> Result<()> {
        if let Some(ref mut f) = self.file {
            let separator = "=".repeat(80);
            
            writeln!(f, "{}", separator)?;
            writeln!(f, "Stage: {}", self.stage_name)?;
            writeln!(f, "Build ID: {}", self.build_id)?;
            writeln!(f, "Start Time: {}", Utc::now().to_rfc3339())?;
            writeln!(f)?;
            writeln!(f, "Command: {}", command)?;
            writeln!(f, "Arguments: {:?}", args)?;
            writeln!(f, "Working Directory: {:?}", std::env::current_dir().ok())?;
            
            writeln!(f, "Environment (safe):")?;
            for (key, value) in std::env::vars() {
                if !is_sensitive_env(&key) {
                    writeln!(f, "  {}={}", key, value)?;
                } else {
                    writeln!(f, "  {}=***REDACTED***", key)?;
                }
            }
            
            writeln!(f, "{}", separator)?;
            writeln!(f)?;
            writeln!(f, "--- STDOUT ---")?;
            writeln!(f)?;
            
            f.flush()?;
        }
        Ok(())
    }

    pub fn write_stdout(&mut self, data: &str) -> Result<()> {
        if let Some(ref mut f) = self.file {
            write!(f, "{}", data)?;
            f.flush()?;
        }
        Ok(())
    }

    pub fn write_stderr(&mut self, data: &str) -> Result<()> {
        if let Some(ref mut f) = self.file {
            writeln!(f, "\n--- STDERR ---")?;
            writeln!(f, "{}", data)?;
            writeln!(f, "--- END STDERR ---\n")?;
            f.flush()?;
        }
        Ok(())
    }

    pub fn write_footer(&mut self, output: &ProcessOutput) -> Result<()> {
        if let Some(ref mut f) = self.file {
            writeln!(f)?;
            let separator = "=".repeat(80);
            
            writeln!(f, "{}", separator)?;
            writeln!(f, "Process Completed")?;
            writeln!(f)?;
            writeln!(f, "Exit Code: {:?}", output.exit_code)?;
            writeln!(f, "Success: {}", output.success)?;
            writeln!(f, "Timed Out: {}", output.timed_out)?;
            writeln!(f, "Duration: {:?}", output.duration)?;
            writeln!(f, "End Time: {}", output.end_time.map_or("N/A".to_string(), |t| t.to_rfc3339()))?;
            writeln!(f)?;
            writeln!(f, "Statistics:")?;
            writeln!(f, "  Stdout Lines: {}", output.stdout.lines().count())?;
            writeln!(f, "  Stdout Bytes: {} bytes", output.stdout.len())?;
            writeln!(f, "  Stderr Lines: {}", output.stderr.lines().count())?;
            writeln!(f, "  Stderr Bytes: {} bytes", output.stderr.len())?;
            writeln!(f, "{}", separator)?;
            
            f.flush()?;
        }
        Ok(())
    }

    pub fn get_log_path(&self) -> &Path {
        &self.log_file
    }
}

fn is_sensitive_env(key: &str) -> bool {
    let sensitive_keys = [
        "PASSWORD", "SECRET", "TOKEN", "KEY", "API_KEY", "PRIVATE",
        "CREDENTIAL", "AUTH", "PASSWD", "GPG_KEY", "SSH_KEY",
    ];

    sensitive_keys.iter().any(|s| key.to_uppercase().contains(s))
}

impl Drop for LogFileWriter {
    fn drop(&mut self) {
        if let Some(ref mut f) = self.file {
            let _ = writeln!(f, "\n[Log file closed at {}]", Utc::now().to_rfc3339());
            let _ = f.flush();
        }
    }
}

pub struct StreamDispatcher {
    stage_status: StageStatus,
    log_writer: Option<Arc<Mutex<LogFileWriter>>>,
    log_level: LogLevel,
}

impl Clone for StreamDispatcher {
    fn clone(&self) -> Self {
        StreamDispatcher {
            stage_status: self.stage_status.clone(),
            log_writer: self.log_writer.clone(),
            log_level: self.log_level.clone(),
        }
    }
}

impl StreamDispatcher {
    pub fn new(log_level: LogLevel, stage_name: &str) -> Self {
        StreamDispatcher {
            stage_status: StageStatus::new(stage_name),
            log_writer: None,
            log_level,
        }
    }

    pub fn with_log_writer(mut self, writer: Arc<Mutex<LogFileWriter>>) -> Self {
        self.log_writer = Some(writer);
        self
    }

    pub fn get_log_level(&self) -> &LogLevel {
        &self.log_level
    }

    pub fn update_status(&mut self, status: &str) {
        self.stage_status.set_status(status);
        
        if self.log_level.is_silent() {
            return;
        }

        print_stage_status(&self.stage_status, &self.log_level);
    }

    pub fn update_status_with_command(&mut self, status: &str, command: &str) {
        let status_with_cmd = format!("{} ({})", status, command);
        self.update_status(&status_with_cmd);
    }

    pub fn update_substage(&mut self, substage: &str) {
        self.stage_status.set_substage(substage);

        if !self.log_level.should_show_substage() {
            return;
        }

        print_stage_status(&self.stage_status, &self.log_level);
    }

    fn should_update_periodic_status(&self) -> bool {
        if self.log_level.is_silent() || self.log_level.should_passthrough() {
            return false;
        }

        self.stage_status.since_last_update() >= self.log_level.status_update_interval()
    }

    fn should_show_heartbeat(&self) -> bool {
        if self.log_level.is_silent() {
            return false;
        }

        self.stage_status.since_last_output() >= self.log_level.heartbeat_interval()
    }

    fn dispatch_to_console(&self, line: &str) {
        match self.log_level {
            LogLevel::Silent => {}
            LogLevel::Normal => {}
            LogLevel::Verbose => {
                if self.log_level.is_key_log_line(line) {
                    println!("  {}", line.trim_end());
                }
            }
            LogLevel::Detailed => {
                if !self.log_level.is_noise_line(line) {
                    println!("  {}", line.trim_end());
                }
            }
            LogLevel::Trace => {
                if !self.log_level.is_noise_line(line) {
                    println!("  {}", line.trim_end());
                }
            }
            LogLevel::Debug => {
                print!("{}", line);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        }
    }

    pub fn dispatch_stdout(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }

        self.stage_status.increment_line();

        self.record_to_log_file_stdout(data);

        self.filter_to_console_stdout(data);

        self.update_periodic_status();
    }

    pub fn dispatch_stderr(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }

        self.record_to_log_file_stderr(data);
        self.filter_to_console_stderr(data);
    }

    fn record_to_log_file_stdout(&self, data: &str) {
        if let Some(ref writer) = self.log_writer {
            if let Ok(mut w) = writer.lock() {
                let _ = w.write_stdout(data);
            }
        }
    }

    fn record_to_log_file_stderr(&self, data: &str) {
        if let Some(ref writer) = self.log_writer {
            if let Ok(mut w) = writer.lock() {
                let _ = w.write_stderr(data);
            }
        }
    }

    fn filter_to_console_stdout(&mut self, data: &str) {
        if self.log_level == LogLevel::Debug {
            print!("{}", data);
            use std::io::Write;
            let _ = std::io::stdout().flush();
            return;
        }

        for line in data.lines() {
            if !line.trim().is_empty() {
                self.dispatch_to_console(line);
            }
        }
    }

    fn filter_to_console_stderr(&self, data: &str) {
        if self.log_level == LogLevel::Debug {
            eprint!("{}", data);
            use std::io::Write;
            let _ = std::io::stderr().flush();
        } else if self.log_level >= LogLevel::Verbose {
            for line in data.lines().take(10) {
                eprintln!("[ERR] {}", line);
            }
            if data.lines().count() > 10 {
                eprintln!("[ERR] ... and more stderr output");
            }
        }
    }

    fn update_periodic_status(&mut self) {
        if self.log_level == LogLevel::Silent {
            return;
        }

        if self.should_update_periodic_status() {
            let lines_processed = self.stage_status.line_count;
            self.update_status(&format!("running... ({} lines)", lines_processed));
        } else if self.should_show_heartbeat() && self.log_level <= LogLevel::Normal {
            println!("[{}]: still running ({})",
                self.stage_status.name.to_uppercase(),
                self.stage_status.format_elapsed()
            );
        }
    }

    pub fn finish(&mut self, output: &ProcessOutput) -> Result<()> {
        if self.log_level != LogLevel::Debug {
            print_stage_complete(&self.stage_status, output.success, &self.log_level);
        }

        if !output.success {
            if let Some(ref writer) = self.log_writer {
                if let Ok(w) = writer.lock() {
                    print_error_summary(
                        &self.stage_status.name,
                        output,
                        w.get_log_path(),
                        &self.log_level
                    );
                }
            }
        }

        if let Some(ref writer) = self.log_writer {
            if let Ok(mut w) = writer.lock() {
                w.write_footer(output)?;
            }
        }

        Ok(())
    }

    pub fn get_stage_name(&self) -> &str {
        &self.stage_status.name
    }
}
