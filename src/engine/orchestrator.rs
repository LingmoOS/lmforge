use std::sync::Arc;
use std::path::PathBuf;
use anyhow::{Result, bail};
use tracing::{info, warn, error};

use crate::domain::context::{BuildContext, BuildConfig};
use crate::domain::config::{ConfigLoader, PartialConfig};
use crate::stages::stage::Stage;
use crate::stages::pipeline::Pipeline;
use crate::platform::platform_trait::Platform;
use crate::platform::debian::DebianPlatform;
use crate::engine::engine_trait::ImageEngine;
use crate::engine::livebuild::LiveBuildEngine;
use crate::features::feature_trait::Feature;
use crate::features::desktop::DesktopFeature;
use crate::features::live::LiveFeature;
use crate::features::installer::InstallerFeature;
use crate::infra::OverlayManager;

use super::cli::Cli;

pub struct BuildOrchestrator {
    target: String,
    profile: Option<String>,
    features: Vec<String>,
    clean: bool,
}

impl BuildOrchestrator {
    pub fn new() -> Self {
        BuildOrchestrator {
            target: String::new(),
            profile: None,
            features: vec![],
            clean: false,
        }
    }

    pub fn with_target(mut self, target: &str) -> Self {
        self.target = target.to_string();
        self
    }

    pub fn with_profile(mut self, profile: Option<&str>) -> Self {
        self.profile = profile.map(|s| s.to_string());
        self
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn with_clean(mut self, clean: bool) -> Self {
        self.clean = clean;
        self
    }

    pub async fn run(&self, cli: &Cli) -> Result<()> {
        info!("Starting build orchestration for target '{}'", self.target);

        let config = self.load_config(cli)?;
        
        if self.clean {
            self.cleanup_workspace(&config).await?;
        }

        let mut ctx = BuildContext::new(config)?;

        info!("Build context initialized");
        info!("Architecture: {}", ctx.arch());
        info!("Suite: {}", ctx.suite());
        info!("Output directory: {:?}", ctx.output_path());

        let platform = self.create_platform(&ctx)?;
        platform.validate_environment().await?;

        let image_engine = self.create_image_engine(&ctx)?;

        let pipeline = self.build_pipeline(&mut ctx, &*platform, &*image_engine).await?;

        let completed_stages = pipeline.execute(&mut ctx).await?;

        info!("Build completed successfully");
        info!("Completed stages: {:?}", completed_stages);

        let artifacts = ctx.get_artifacts().await;
        if !artifacts.is_empty() {
            info!("Generated {} artifacts:", artifacts.len());
            for artifact in &artifacts {
                info!(
                    "  - {:?}: {} ({} bytes)",
                    artifact.kind,
                    artifact.filename(),
                    artifact.size
                );
            }
        }

        image_engine.cleanup(&mut ctx).await?;

        Ok(())
    }

    fn load_config(&self, cli: &Cli) -> Result<BuildConfig> {
        info!("Loading configuration");

        let mut partial_config = PartialConfig::default();

        if let Some(ref arch) = cli.arch {
            partial_config.arch = Some(arch.clone());
        }
        if let Some(ref suite) = cli.suite {
            partial_config.suite = Some(suite.clone());
        }
        if let Some(ref output) = cli.output {
            partial_config.output_dir = Some(output.clone());
        }
        if let Some(ref workspace) = cli.workspace {
            partial_config.workspace_dir = Some(workspace.clone());
        }
        if !self.features.is_empty() {
            partial_config.features = Some(self.features.clone());
        }

        let loader = ConfigLoader::new()
            .with_builtin()?
            .with_preset(self.profile.as_deref().unwrap_or("official"))?
            .with_etc_config()?
            .with_user_config(&cli.config.clone().unwrap_or_default())
            .with_cli_overrides(&partial_config);

        let config = loader.merge();

        debug!("Configuration loaded successfully");
        Ok(config)
    }

    async fn cleanup_workspace(&self, config: &BuildConfig) -> Result<()> {
        info!("Cleaning up workspace");

        if config.workspace_dir.exists() {
            tokio::fs::remove_dir_all(&config.workspace_dir).await?;
        }

        if config.output_dir.exists() {
            // Only clean specific build artifacts, not the entire output dir
            // to preserve user data
            for entry in std::fs::read_dir(&config.output_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map_or(false, |ext| ext == "iso" || ext == "zst") {
                    tokio::fs::remove_file(&path).await?;
                }
            }
        }

        Ok(())
    }

    fn create_platform(&self, _ctx: &BuildContext) -> Result<Arc<dyn Platform>> {
        info!("Creating platform instance");

        // For V1, we only support Debian
        // In future, this would be configurable based on config.platform.name
        let platform: Arc<dyn Platform> = Arc::new(
            DebianPlatform::new(_ctx.suite())
                .with_components(_ctx.config.platform.components.clone())
        );

        Ok(platform)
    }

    fn create_image_engine(&self, ctx: &BuildContext) -> Result<Arc<dyn ImageEngine>> {
        info!("Creating image engine");

        match &ctx.config.image.engine {
            crate::domain::context::ImageEngineType::LiveBuild => {
                let engine: Arc<dyn ImageEngine> = Arc::new(
                    LiveBuildEngine::new(ctx.workspace.temp.join("lb"))
                );
                Ok(engine)
            }
            crate::domain::context::ImageEngineType::Native => {
                // Native engine not yet implemented in V1
                bail!("Native image engine is not yet implemented. Use live-build engine.");
            }
        }
    }

    async fn build_pipeline(
        &self,
        ctx: &mut BuildContext,
        platform: &dyn Platform,
        image_engine: &dyn ImageEngine,
    ) -> Result<Pipeline> {
        info!("Building pipeline");

        let mut stages: Vec<Box<dyn Stage>> = Vec::new();

        stages.push(Box::new(BootstrapStage {
            platform_name: platform.name().to_string(),
        }));

        stages.push(Box::new(PackagesStage));

        stages.push(Box::new(OverlayStage));

        stages.push(Box::new(ImageStage {
            engine_name: image_engine.name().to_string(),
        }));

        stages.push(Box::new(MetadataStage));
        stages.push(Box::new(ReleaseStage));

        let feature_instances = self.create_feature_instances()?;
        for feature in &feature_instances {
            feature.register_stages(&mut stages).await?;
            feature.prepare_overlay(ctx).await?;
        }

        let pipeline = Pipeline::with_stages(stages)?;

        info!(
            "Pipeline created with {} stages",
            pipeline.len()
        );
        info!("Stages: {:?}", pipeline.stage_names());

        Ok(pipeline)
    }

    fn create_feature_instances(&self) -> Result<Vec<Box<dyn Feature>>> {
        let mut features: Vec<Box<dyn Feature>> = Vec::new();

        for feature_name in &self.features {
            match feature_name.as_str() {
                "desktop" => {
                    features.push(Box::new(DesktopFeature::gnome()));
                    info!("Enabled desktop feature");
                }
                "live" => {
                    features.push(Box::new(LiveFeature::new()));
                    info!("Enabled live feature");
                }
                "installer" => {
                    features.push(Box::new(InstallerFeature::new()));
                    info!("Enabled installer feature");
                }
                "secureboot" => {
                    warn!("SecureBoot feature not yet implemented");
                }
                other => {
                    warn!("Unknown feature '{}' ignored", other);
                }
            }
        }

        Ok(features)
    }
}

struct BootstrapStage {
    platform_name: String,
}

#[async_trait]
impl Stage for BootstrapStage {
    fn name(&self) -> &str {
        "bootstrap"
    }

    fn description(&self) -> &str {
        "Bootstrap base system using debootstrap"
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        use crate::platform::debian::DebianPlatform;

        info!("Running bootstrap stage on platform: {}", self.platform_name);

        let platform = DebianPlatform::new(ctx.suite())
            .with_components(ctx.config.platform.components.clone());

        platform.bootstrap(ctx).await?;

        info!("Bootstrap completed");
        Ok(())
    }
}

struct PackagesStage;

#[async_trait]
impl Stage for PackagesStage {
    fn name(&self) -> &str {
        "packages"
    }

    fn description(&self) -> &str {
        "Install base packages into rootfs"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["bootstrap"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Installing base packages");

        let packages = OverlayManager::load_package_list(&ctx.workspace.overlay)?;
        
        if !packages.is_empty() {
            use crate::platform::debian::DebianPlatform;
            
            let platform = DebianPlatform::new(ctx.suite());
            let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            
            platform.install_packages(ctx, &pkg_refs).await?;
        } else {
            debug!("No additional packages to install from overlay");
        }

        info!("Package installation completed");
        Ok(())
    }
}

struct OverlayStage;

#[async_trait]
impl Stage for OverlayStage {
    fn name(&self) -> &str {
        "overlay"
    }

    fn description(&self) -> &str {
        "Apply filesystem overlays and execute hooks"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["packages"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Applying overlays");

        OverlayManager::apply_overlays(ctx).await?;

        info!("Overlays applied");
        Ok(())
    }
}

struct ImageStage {
    engine_name: String,
}

#[async_trait]
impl Stage for ImageStage {
    fn name(&self) -> &str {
        "image"
    }

    fn description(&self) -> &str {
        "Generate image using configured image engine"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["overlay"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Building image with engine: {}", self.engine_name);

        match ctx.config.image.engine.clone() {
            crate::domain::context::ImageEngineType::LiveBuild => {
                let engine = LiveBuildEngine::new(ctx.workspace.temp.join("lb"));
                
                engine.prepare(ctx).await?;
                engine.build(ctx).await?;
            }
            crate::domain::context::ImageEngineType::Native => {
                return Err(anyhow::anyhow!("Native engine not yet implemented"));
            }
        }

        info!("Image generation completed");
        Ok(())
    }
}

struct MetadataStage;

#[async_trait]
impl Stage for MetadataStage {
    fn name(&self) -> &str {
        "metadata"
    }

    fn description(&self) -> &str {
        "Generate manifest and checksum files"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["image"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Generating metadata");

        ctx.ensure_output_dir()?;

        let artifacts = ctx.get_artifacts().await;
        
        if !artifacts.is_empty() {
            let manifest_path = ctx.output_path().join("MANIFEST");
            let checksum_path = ctx.output_path().join("SHA256SUMS");

            let mut manifest_content = String::new();
            let mut checksum_content = String::new();

            for artifact in &artifacts {
                manifest_content.push_str(&artifact.to_manifest_entry());
                manifest_content.push('\n');

                checksum_content.push_str(&format!(
                    "{}  {}\n",
                    artifact.checksum,
                    artifact.filename()
                ));
            }

            tokio::fs::write(&manifest_path, manifest_content).await?;
            tokio::fs::write(&checksum_path, checksum_content).await?;

            info!("Manifest written to {:?}", manifest_path);
            info!("Checksums written to {:?}", checksum_path);
        }

        info!("Metadata generation completed");
        Ok(())
    }
}

struct ReleaseStage;

#[async_trait]
impl Stage for ReleaseStage {
    fn name(&self) -> &str {
        "release"
    }

    fn description(&self) -> &str {
        "Finalize release and generate build information"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["metadata"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        info!("Finalizing release");

        let build_info = serde_json::json!({
            "version": ctx.version(),
            "arch": ctx.arch(),
            "suite": ctx.suite(),
            "build_time": chrono::Utc::now().to_rfc3339(),
            "stages_completed": ctx.runtime_state.stages_completed,
            "artifacts_count": ctx.get_artifacts().await.len(),
            "lmforge_version": env!("CARGO_PKG_VERSION"),
        });

        let build_info_path = ctx.output_path().join("BUILDINFO.json");
        tokio::fs::write(
            &build_info_path,
            serde_json::to_string_pretty(&build_info)?
        ).await?;

        info!("Build info written to {:?}", build_info_path);
        info!("Release finalized successfully");

        println!("\n✓ Build completed successfully!");
        println!("  Output directory: {:?}", ctx.output_path());
        println!("  Artifacts: {}", ctx.get_artifacts().await.len());
        println!("  Stages completed: {:?}", ctx.runtime_state.stages_completed);

        Ok(())
    }
}
