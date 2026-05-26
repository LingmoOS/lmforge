use std::sync::Arc;
use std::path::PathBuf;
use std::time::Instant;
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
use crate::telemetry::build_id::BuildId as BuildIdStruct;
use crate::telemetry::runtime::RuntimeLogger;

use super::cli::Cli;

pub struct BuildOrchestrator {
    target: String,
    profile: Option<String>,
    features: Vec<String>,
    clean: bool,
    build_id: BuildIdStruct,
}

impl BuildOrchestrator {
    pub fn new() -> Self {
        BuildOrchestrator {
            target: String::new(),
            profile: None,
            features: vec![],
            clean: false,
            build_id: BuildIdStruct::new(),
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
        let logger = RuntimeLogger::new(&self.build_id.id);
        
        info!(
            target: "lmforge_workspace",
            build_id = %self.build_id,
            target = %self.target,
            "starting build orchestration"
        );

        let config = self.load_config(cli)?;
        
        if self.clean {
            logger.log_workspace_cleanup(&config.workspace_dir);
            self.cleanup_workspace(&config).await?;
        }

        logger.log_workspace_create(&config.workspace_dir);
        let mut ctx = BuildContext::new(config)?;

        stage_info!("workspace", 
            arch = %ctx.arch(),
            suite = %ctx.suite(),
            output = ?ctx.output_path(),
            build_id = %self.build_id,
            "build context initialized"
        );

        let platform = self.create_platform(&ctx)?;
        platform.validate_environment().await?;

        let image_engine = self.create_image_engine(&ctx)?;

        let pipeline = self.build_pipeline(&mut ctx, &*platform, &*image_engine).await?;

        let start_time = Instant::now();
        
        let completed_stages = pipeline.execute(&mut ctx).await?;
        
        let duration = start_time.elapsed();

        stage_info!("release",
            stages_completed = completed_stages.len(),
            total_stages = pipeline.len(),
            duration_secs = duration.as_secs_f64(),
            build_id = %self.build_id,
            "build completed successfully"
        );

        let artifacts = ctx.get_artifacts().await;
        if !artifacts.is_empty() {
            for artifact in &artifacts {
                stage_info!("release",
                    artifact_kind = ?artifact.kind,
                    filename = %artifact.filename(),
                    size_bytes = artifact.size,
                    checksum = &artifact.checksum[..16],
                    "generated artifact"
                );
            }
        }

        image_engine.cleanup(&mut ctx).await?;

        Ok(())
    }

    fn load_config(&self, cli: &Cli) -> Result<BuildConfig> {
        debug!(target: "lmforge_workspace", build_id = %self.build_id, "loading configuration");

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

        debug!(target: "lmforge_workspace", "configuration loaded");
        Ok(config)
    }

    async fn cleanup_workspace(&self, config: &BuildConfig) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);

        if config.workspace_dir.exists() {
            tokio::fs::remove_dir_all(&config.workspace_dir).await?;
        }

        if config.output_dir.exists() {
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
        stage_info!("workspace",
            platform_name = _ctx.config.platform.name,
            "creating platform instance"
        );

        let platform: Arc<dyn Platform> = Arc::new(
            DebianPlatform::new(_ctx.suite())
                .with_components(_ctx.config.platform.components.clone())
        );

        Ok(platform)
    }

    fn create_image_engine(&self, ctx: &BuildContext) -> Result<Arc<dyn ImageEngine>> {
        match &ctx.config.image.engine {
            crate::domain::context::ImageEngineType::LiveBuild => {
                stage_info!("image", engine_type = "live-build", "creating image engine");
                
                let engine: Arc<dyn ImageEngine> = Arc::new(
                    LiveBuildEngine::new(ctx.workspace.temp.join("lb"))
                );
                Ok(engine)
            }
            crate::domain::context::ImageEngineType::Native => {
                stage_error!("image", "native engine not yet implemented");
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
        stage_info!("workspace", "building pipeline");

        let mut stages: Vec<Box<dyn Stage>> = Vec::new();

        stages.push(Box::new(BootstrapStage {
            platform_name: platform.name().to_string(),
            build_id: self.build_id.clone(),
        }));

        stages.push(Box::new(PackagesStage {
            build_id: self.build_id.clone(),
        }));

        stages.push(Box::new(OverlayStage {
            build_id: self.build_id.clone(),
        }));

        stages.push(Box::new(ImageStage {
            engine_name: image_engine.name().to_string(),
            build_id: self.build_id.clone(),
        }));

        stages.push(Box::new(MetadataStage {
            build_id: self.build_id.clone(),
        }));
        
        stages.push(Box::new(ReleaseStage {
            build_id: self.build_id.clone(),
        }));

        let feature_instances = self.create_feature_instances()?;
        for feature in &feature_instances {
            feature.register_stages(&mut stages).await?;
            feature.prepare_overlay(ctx).await?;
        }

        let pipeline = Pipeline::with_stages(stages)?;

        stage_info!("workspace",
            stage_count = pipeline.len(),
            stages = ?pipeline.stage_names(),
            "pipeline created"
        );

        Ok(pipeline)
    }

    fn create_feature_instances(&self) -> Result<Vec<Box<dyn Feature>>> {
        let mut features: Vec<Box<dyn Feature>> = Vec::new();

        for feature_name in &self.features {
            match feature_name.as_str() {
                "desktop" => {
                    features.push(Box::new(DesktopFeature::gnome()));
                    stage_info!("packages", feature = "desktop", "enabled desktop feature");
                }
                "live" => {
                    features.push(Box::new(LiveFeature::new()));
                    stage_info!("image", feature = "live", "enabled live feature");
                }
                "installer" => {
                    features.push(Box::new(InstallerFeature::new()));
                    stage_info!("image", feature = "installer", "enabled installer feature");
                }
                "secureboot" => {
                    stage_warn!("image", feature = "secureboot", "SecureBoot not yet implemented");
                }
                other => {
                    stage_warn!("workspace", feature = other, "unknown feature ignored");
                }
            }
        }

        Ok(features)
    }
}

struct BootstrapStage {
    platform_name: String,
    build_id: BuildIdStruct,
}

#[async_trait]
impl Stage for BootstrapStage {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Bootstrap base system using debootstrap"
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        use crate::platform::debian::DebianPlatform;

        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("workspace");

        let start_time = Instant::now();

        let platform = DebianPlatform::new(ctx.suite())
            .with_components(ctx.config.platform.components.clone());

        platform.bootstrap(ctx).await?;

        let duration = start_time.elapsed();
        logger.log_stage_complete("workspace", duration);

        Ok(())
    }
}

struct PackagesStage {
    build_id: BuildIdStruct,
}

#[async_trait]
impl Stage for PackagesStage {
    fn name(&self) -> &str {
        "packages"
    }

    fn description(&self) -> &str {
        "Install base packages into rootfs"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["workspace"]
    }

    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("packages");

        let start_time = Instant::now();

        let packages = OverlayManager::load_package_list(&ctx.workspace.overlay)?;
        
        if !packages.is_empty() {
            use crate::platform::debian::DebianPlatform;
            
            let platform = DebianPlatform::new(ctx.suite());
            let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
            
            platform.install_packages(ctx, &pkg_refs).await?;
            
            stage_info!("packages", package_count = packages.len(), "installed packages");
        } else {
            debug!(target: "lmforge_packages", "no additional packages to install");
        }

        let duration = start_time.elapsed();
        logger.log_stage_complete("packages", duration);

        Ok(())
    }
}

struct OverlayStage {
    build_id: BuildIdStruct,
}

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
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("overlay");

        let start_time = Instant::now();

        OverlayManager::apply_overlays(ctx).await?;

        let duration = start_time.elapsed();
        logger.log_stage_complete("overlay", duration);

        Ok(())
    }
}

struct ImageStage {
    engine_name: String,
    build_id: BuildIdStruct,
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
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("image");

        let start_time = Instant::now();

        stage_info!(target: "lmforge_image",
            engine = %self.engine_name,
            "generating image"
        );

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

        let duration = start_time.elapsed();
        logger.log_stage_complete("image", duration);

        Ok(())
    }
}

struct MetadataStage {
    build_id: BuildIdStruct,
}

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
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("metadata");

        let start_time = Instant::now();

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

            stage_info!(target: "lmforge_release",
                manifest = ?manifest_path,
                checksums = ?checksum_path,
                "metadata generated"
            );
        }

        let duration = start_time.elapsed();
        logger.log_stage_complete("metadata", duration);

        Ok(())
    }
}

struct ReleaseStage {
    build_id: BuildIdStruct,
}

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
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("release");

        let start_time = Instant::now();

        let build_info = serde_json::json!({
            "version": ctx.version(),
            "arch": ctx.arch(),
            "suite": ctx.suite(),
            "build_time": chrono::Utc::now().to_rfc3339(),
            "stages_completed": ctx.runtime_state.stages_completed,
            "artifacts_count": ctx.get_artifacts().await.len(),
            "lmforge_version": env!("CARGO_PKG_VERSION"),
            "build_id": self.build_id.id,
        });

        let build_info_path = ctx.output_path().join("BUILDINFO.json");
        tokio::fs::write(
            &build_info_path,
            serde_json::to_string_pretty(&build_info)?
        ).await?;

        stage_info!(target: "lmforge_release",
            buildinfo = ?build_info_path,
            artifacts_count = ctx.get_artifacts().await.len(),
            stages = ?ctx.runtime_state.stages_completed,
            "release finalized"
        );

        let duration = start_time.elapsed();
        logger.log_stage_complete("release", duration);

        Ok(())
    }
}
