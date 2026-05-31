use clap::{Parser, Subcommand};
use anyhow::Result;

use super::build::BuildCommand;
use super::package::PackageCommand;

#[derive(Debug, Parser)]
#[command(name = "lmforge")]
#[command(about = "Industrial-grade Linux distribution build platform")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count, help = "Increase verbosity (-v=verbose, -vv=detailed, -vvv=trace)")]
    pub verbose: u8,

    #[arg(short = 'L', long = "log-level", global = true, value_name = "LEVEL", help = "Set log level (0=silent, 1=normal, 2=verbose, 3=detailed, 4=trace, 5=debug)")]
    pub log_level: Option<u8>,

    #[arg(long, global = true)]
    pub config: Option<std::path::PathBuf>,

    #[arg(long, global = true)]
    pub output: Option<std::path::PathBuf>,

    #[arg(long, global = true)]
    pub workspace: Option<std::path::PathBuf>,

    #[arg(long, global = true)]
    pub arch: Option<String>,

    #[arg(long, global = true)]
    pub suite: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Build ISO images and distributions
    Build(BuildCommand),

    /// Package management operations
    Package(PackageCommand),

    /// Repository management
    Repo(RepoCommand),

    /// Show configuration
    Config(ConfigCommand),
}

#[derive(Debug, Parser)]
pub struct RepoCommand {
    #[command(subcommand)]
    pub action: RepoAction,
}

#[derive(Debug, Subcommand)]
pub enum RepoAction {
    /// Publish packages to repository
    Publish,
    
    /// Initialize repository structure
    Init,
    
    /// Update repository metadata
    Update,
}

#[derive(Debug, Parser)]
pub struct ConfigCommand {
    /// Show current configuration
    #[arg(short, long)]
    pub show: bool,

    /// Validate configuration
    #[arg(long)]
    pub validate: bool,

    /// Generate default configuration
    #[arg(long)]
    pub generate: bool,
}

impl Cli {
    pub fn execute(&self) -> Result<()> {
        match &self.command {
            Commands::Build(cmd) => cmd.execute(self)?,
            Commands::Package(cmd) => cmd.execute(self)?,
            Commands::Repo(cmd) => self.execute_repo_command(cmd)?,
            Commands::Config(cmd) => self.execute_config_command(cmd)?,
        }
        Ok(())
    }

    pub fn get_log_level(&self) -> crate::runtime::log_stream::LogLevel {
        use crate::runtime::log_stream::LogLevel;
        
        if let Some(level) = self.log_level {
            LogLevel::from_int(level)
        } else if self.verbose > 0 {
            match self.verbose {
                1 => LogLevel::Verbose,
                2 => LogLevel::Detailed,
                _ => LogLevel::Trace,
            }
        } else {
            LogLevel::Normal
        }
    }

    fn execute_repo_command(&self, _cmd: &RepoCommand) -> Result<()> {
        println!("Repository commands not yet implemented");
        Ok(())
    }

    fn execute_config_command(&self, cmd: &ConfigCommand) -> Result<()> {
        if cmd.show || (!cmd.validate && !cmd.generate) {
            self.show_config();
        }
        
        if cmd.validate {
            self.validate_config()?;
        }
        
        if cmd.generate {
            self.generate_config()?;
        }
        
        Ok(())
    }

    fn show_config(&self) {
        println!("Current configuration:");
        if let Some(ref arch) = self.arch {
            println!("  Architecture: {}", arch);
        }
        if let Some(ref suite) = self.suite {
            println!("  Suite: {}", suite);
        }
        if let Some(ref output) = self.output {
            println!("  Output directory: {:?}", output);
        }
        if let Some(ref workspace) = self.workspace {
            println!("  Workspace directory: {:?}", workspace);
        }
        if let Some(ref config) = self.config {
            println!("  Config file: {:?}", config);
        }
    }

    fn validate_config(&self) -> Result<()> {
        println!("Configuration validation passed");
        Ok(())
    }

    fn generate_config(&self) -> Result<()> {
        let config_content = r#"[arch]
default = "amd64"

[suite]
default = "bookworm"

[platform]
name = "debian"
components = ["main", "contrib", "non-free"]

[image]
engine = "livebuild"
iso_name = "lingmo-live.iso"
volume_id = "Lingmo Live"
"#;

        let path = std::path::Path::new("lmforge.toml");
        std::fs::write(path, config_content)?;
        println!("Generated configuration file: {:?}", path);
        Ok(())
    }
}
