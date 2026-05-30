use anyhow::{Result};
use tracing::{info, debug, warn};

use super::platform_trait::Platform;
use crate::domain::context::BuildContext;

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

    pub fn get_mirror_url(&self) -> &str {
        self.mirror.as_deref().unwrap_or("http://mirrors.tuna.tsinghua.edu.cn/debian")
    }

    pub fn get_suite(&self) -> &str {
        &self.suite
    }

    pub fn get_components(&self) -> &Vec<String> {
        &self.components
    }
}

impl Platform for DebianPlatform {
    fn name(&self) -> &str {
        "debian"
    }

    fn bootstrap(&self, _ctx: &mut BuildContext) -> Result<()> {
        info!(
            target: "lmforge_platform",
            platform = %self.name(),
            suite = %self.suite,
            "V1 Phase: Delegating bootstrap to LiveBuildEngine"
        );

        info!(
            target: "lmforge_platform",
            note = "live-build internally handles debootstrap/mmdebstrap",
            "No direct bootstrap execution in V1 architecture"
        );

        debug!(
            target: "lmforge_platform",
            mirror = %self.get_mirror_url(),
            components = ?self.components,
            "Platform configuration prepared for live-build"
        );

        Ok(())
    }

    fn install_packages(&self, _ctx: &mut BuildContext, packages: &[&str]) -> Result<()> {
        info!(
            target: "lmforge_platform",
            platform = %self.name(),
            packages = ?packages,
            "V1 Phase: Package installation delegated to live-build package-lists"
        );

        info!(
            target: "lmforge_platform",
            note = "packages will be installed via live-build hooks or chroot_local-packages",
            "Use OverlayManager to configure package-lists/"
        );

        debug!(
            target: "lmforge_platform",
            package_count = packages.len(),
            "Package list recorded for overlay synchronization"
        );

        Ok(())
    }

    fn generate_repo_metadata(&self, _ctx: &mut BuildContext) -> Result<()> {
        info!(
            target: "lmforge_platform",
            platform = %self.name(),
            "V1 Phase: Repository metadata generation delegated to live-build"
        );

        info!(
            target: "lmforge_platform",
            note = "apt sources and repository configuration handled by lb config",
            "Repository metadata will be generated during live-build process"
        );

        Ok(())
    }

    fn supported_architectures(&self) -> Vec<&str> {
        vec!["amd64", "i386", "arm64", "armhf"]
    }

    fn supported_suites(&self) -> Vec<&str> {
        vec![
            "stable",
            "testing",
            "unstable",
            "bookworm",
            "trixie",
            "sid",
        ]
    }

    fn package_manager_command(&self) -> &str {
        "apt-get"
    }

    fn validate_environment(&self) -> Result<()> {
        info!(
            target: "lmforge_platform",
            platform = %self.name(),
            "Validating Debian platform environment for V1 (live-build mode)"
        );

        let rt = tokio::runtime::Runtime::new()?;

        let lb_exists = rt.block_on(async {
            crate::runtime::process::Executor::exists("lb").await
        });

        if !lb_exists {
            let lb_build_exists = rt.block_on(async {
                crate::runtime::process::Executor::exists("lb_build").await
            });

            if !lb_build_exists {
                warn!(
                    target: "lmforge_platform",
                    "live-build not found. Install with: sudo apt-get install live-build"
                );
                
                return Err(anyhow::anyhow!(
                    "live-build is required but not installed.\n\
                     \n\
                     Installation:\n\
                     sudo apt-get update\n\
                     sudo apt-get install live-build\n\
                     \n\
                     V1 Architecture requires live-build for ISO generation."
                ));
            }
        }

        info!(
            target: "lmforge_platform",
            "Debian platform environment validated successfully"
        );

        Ok(())
    }

    fn bootstrap_command(&self) -> &str {
        info!(
            target: "lmforge_platform",
            note = "deprecated in V1 - live-build handles bootstrapping internally",
            "Returning placeholder for API compatibility"
        );
        
        "lb"
    }
}
