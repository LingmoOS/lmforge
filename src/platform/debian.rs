use anyhow::{Result, bail};
use tracing::{info, debug};

use super::platform_trait::Platform;
use crate::domain::context::BuildContext;
use crate::runtime::{process::{Executor, ProcessConfig}, mount::Mount};

pub struct DebianPlatform {
    suite: String,
    mirror: Option<String>,
    components: Vec<String>,
}

impl DebianPlatform {
    pub fn new(suite: impl Into<String>) -> Self {
        DebianPlatform {
            suite: suite.into(),
            mirror: None,
            components: vec!["main".to_string()],
        }
    }

    pub fn with_mirror(mut self, mirror: impl Into<String>) -> Self {
        self.mirror = Some(mirror.into());
        self
    }

    pub fn with_components(mut self, components: Vec<String>) -> Self {
        self.components = components;
        self
    }

    fn get_mirror_url(&self) -> &str {
        self.mirror.as_deref().unwrap_or("http://deb.debian.org/debian")
    }

    fn get_bootstrap_command_args(
        &self,
        ctx: &BuildContext,
        variant: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec![
            "--arch".to_string(),
            ctx.arch().to_string(),
            "--variant".to_string(),
            variant.unwrap_or("minbase").to_string(),
            ctx.suite().to_string(),
            ctx.workspace.rootfs.to_string_lossy().to_string(),
            self.get_mirror_url().to_string(),
        ];

        for component in &self.components {
            args.push(component.clone());
        }

        args
    }
}

impl Platform for DebianPlatform {
    fn name(&self) -> &str {
        "debian"
    }

    fn bootstrap(&self, ctx: &mut BuildContext) -> Result<()> {
        info!(
            "Bootstrapping Debian {} ({}) into {:?}",
            self.suite,
            ctx.arch(),
            ctx.workspace.rootfs
        );

        if ctx.workspace.rootfs.exists() && ctx.workspace.rootfs.read_dir()?.next().is_some() {
            debug!("Rootfs already bootstrapped, skipping");
            return Ok(());
        }

        let args = self.get_bootstrap_command_args(ctx, None);

        let output = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::execute(
                    &ProcessConfig::new(self.bootstrap_command())
                        .args(args)
                        .working_dir(&ctx.workspace.temp)
                ).await
            })?
        };

        match output.status {
            crate::runtime::process::ExitStatus::Success => {
                info!("Debootstrap completed successfully");
                
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    Mount::mount_all_for_chroot(&ctx.workspace.rootfs).await
                })?;
                
                Ok(())
            }
            _ => {
                bail!(
                    "Debootstrap failed:\nstdout: {}\nstderr: {}",
                    output.stdout,
                    output.stderr
                );
            }
        }
    }

    fn install_packages(&self, ctx: &mut BuildContext, packages: &[&str]) -> Result<()> {
        info!("Installing packages: {:?}", packages);

        let mut args = vec![
            "apt-get".to_string(),
            "install".to_string(),
            "-y".to_string(),
            "--no-install-recommends".to_string(),
        ];

        for pkg in packages {
            args.push(pkg.to_string());
        }

        let output = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::execute(
                    &ProcessConfig::new("chroot")
                        .arg(&ctx.workspace.rootfs)
                        .args(args)
                        .env("DEBIAN_FRONTEND", "noninteractive")
                ).await
            })?
        };

        match output.status {
            crate::runtime::process::ExitStatus::Success => {
                debug!("Packages installed successfully");
                Ok(())
            }
            _ => {
                bail!(
                    "Package installation failed:\nstderr: {}",
                    output.stderr
                );
            }
        }
    }

    fn generate_repo_metadata(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Generating repository metadata");

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            Executor::execute_success(
                &ProcessConfig::new("chroot")
                    .arg(&ctx.workspace.rootfs)
                    .args(["apt-get", "update"])
            ).await
        })?;

        Ok(())
    }

    fn supported_architectures(&self) -> Vec<&str> {
        vec!["amd64", "arm64", "i386"]
    }

    fn supported_suites(&self) -> Vec<&str> {
        vec!["bookworm", "bullseye", "sid", "trixie"]
    }

    fn package_manager_command(&self) -> &str {
        "apt-get"
    }

    fn validate_environment(&self) -> Result<()> {
        info!("Validating Debian platform environment");

        let bootstrap_exists = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::exists(self.bootstrap_command()).await
            })
        };

        if !bootstrap_exists {
            bail!(
                "{} is not installed. Please install it first.",
                self.bootstrap_command()
            );
        }

        let dpkg_exists = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                Executor::exists("dpkg").await
            })
        };

        if !dpkg_exists {
            bail!("dpkg is not installed");
        }

        debug!("Environment validation passed");
        Ok(())
    }

    fn bootstrap_command(&self) -> &str {
        "debootstrap"
    }
}
