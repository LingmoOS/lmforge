use std::time::Instant;
use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};

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
use crate::infra::{
    WorkspaceManager,
    WorkspaceLayout,
    OverlayManager,
    ArtifactManager,
    CleanupRecovery,
};
use crate::telemetry::build_id::BuildId as BuildIdStruct;
use crate::telemetry::runtime::RuntimeLogger;

use crate::command::cli::Cli;

use crate::stage_info;
use crate::stage_warn;

pub struct BuildOrchestrator {
    target: String,
    profile: Option<String>,
    features: Vec<String>,
    clean: bool,
    dry_run: bool,
    build_id: BuildIdStruct,
}

impl BuildOrchestrator {
    pub fn new() -> Self {
        BuildOrchestrator {
            target: String::new(),
            profile: None,
            features: vec![],
            clean: false,
            dry_run: false,
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

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn run(&self, cli: &Cli) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        
        info!(
            target: "lmforge_orchestration",
            build_id = %self.build_id,
            target = %self.target,
            dry_run = self.dry_run,
            "starting build orchestration"
        );

        if self.dry_run {
            return self.run_dry_run();
        }

        let config = self.load_config(cli)?;
        
        if self.clean {
            logger.log_workspace_cleanup(&config.workspace_dir);
            self.cleanup_build_dir(&config)?;
        }

        let workspace_manager = WorkspaceManager::new(&config.workspace_dir, &self.build_id.id);
        
        let workspace_layout = workspace_manager.initialize()
            .context("Failed to initialize workspace")?;

        let cleanup_recovery = CleanupRecovery::new(workspace_manager);
        cleanup_recovery.initialize()
            .context("Failed to initialize cleanup and recovery system")?;

        let cleanup_with_ws = cleanup_recovery.with_workspace(workspace_layout.clone());

        let result = self.execute_build(&config, &workspace_layout, &cleanup_with_ws);

        match &result {
            Ok(_) => {
                cleanup_with_ws.mark_completed()
                    .context("Failed to mark build as completed")?;
                
                logger.log_stage_complete("release", Instant::now().duration_since(Instant::now()));
                info!(
                    target: "lmforge_orchestration",
                    build_id = %self.build_id,
                    "build completed successfully"
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                cleanup_with_ws.mark_failed(&error_msg)
                    .context("Failed to mark build as failed")?;
                
                error!(
                    target: "lmforge_orchestration",
                    build_id = %self.build_id,
                    error = %error_msg,
                    "build failed"
                );
                
                cleanup_with_ws.full_cleanup()
                    .context("Failed to perform cleanup after failure")?;
            }
        }

        result
    }

    fn run_dry_run(&self) -> Result<()> {
        stage_info!("workspace",
            mode = "dry-run",
            build_id = %self.build_id,
            "Dry run mode - would execute full build pipeline"
        );

        let stages = [
            ("workspace", "Initialize build workspace"),
            ("packages", "Install base packages"),
            ("overlay", "Apply filesystem overlays"),
            ("image", "Generate ISO with live-build"),
            ("metadata", "Generate manifest and checksums"),
            ("release", "Finalize release artifacts"),
        ];

        for (name, description) in &stages {
            stage_info!(name, description = *description, "[dry-run] would execute");
        }

        stage_info!("release",
            build_id = %self.build_id,
            feature_count = self.features.len(),
            features = ?self.features,
            "Dry run complete"
        );

        Ok(())
    }

    fn execute_build(
        &self,
        config: &BuildConfig,
        workspace_layout: &WorkspaceLayout,
        cleanup: &CleanupRecovery,
    ) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        
        logger.log_workspace_create(&config.workspace_dir);
        
        let mut ctx = BuildContext::new(config.clone())?;
        
        ctx.set_current_stage("initialization");

        stage_info!("workspace",
            arch = %ctx.arch(),
            suite = %ctx.suite(),
            output = ?ctx.output_path(),
            workspace_root = ?workspace_layout.root,
            build_id = %self.build_id,
            "build context initialized"
        );

        let platform = self.create_platform(&ctx)?;
        platform.validate_environment()?;

        let image_engine = LiveBuildEngine::new()
            .with_workspace(workspace_layout.clone());

        let overlay_manager = OverlayManager::new(workspace_layout);
        overlay_manager.initialize()
            .context("Failed to initialize overlays")?;

        let artifact_manager = ArtifactManager::new(workspace_layout, &self.build_id.id);

        ctx.complete_stage("initialization");

        let pipeline = self.build_pipeline(&mut ctx, &*platform, &image_engine)?;
        
        let start_time = Instant::now();
        
        let completed_stages = pipeline.execute(&mut ctx)?;
        
        let duration = start_time.elapsed();

        stage_info!("release",
            stages_completed = completed_stages.len(),
            total_stages = pipeline.len(),
            duration_secs = duration.as_secs_f64(),
            build_id = %self.build_id,
            "build completed successfully"
        );

        let artifacts = ctx.get_artifacts_sync();
        
        if !artifacts.is_empty() {
            self.finalize_artifacts(&artifact_manager, &artifacts, config)?;
            
            for artifact in &artifacts {
                stage_info!("release",
                    artifact_kind = ?artifact.kind,
                    filename = %artifact.filename(),
                    size_bytes = artifact.size,
                    checksum = &artifact.checksum.as_ref().map(|c| &c[..16]).unwrap_or("N/A"),
                    "generated artifact"
                );
            }
        } else {
            warn!(target: "lmforge_release", "no artifacts generated");
        }

        image_engine.cleanup(&mut ctx)?;

        cleanup.cleanup_temp_files()
            .context("Failed to cleanup temporary files")?;

        Ok(())
    }

    fn finalize_artifacts(
        &self,
        artifact_manager: &ArtifactManager,
        artifacts: &[crate::domain::artifact::Artifact],
        config: &BuildConfig,
    ) -> Result<()> {
        info!(target: "lmforge_release", count = artifacts.len(), "finalizing artifacts");

        let config_json = serde_json::to_value(config)?;

        let _checksums_file = artifact_manager.generate_checksums_file(artifacts)
            .context("Failed to generate SHA256SUMS file")?;

        let _manifest_file = artifact_manager.generate_build_manifest(artifacts, &config_json)
            .context("Failed to generate build manifest")?;

        let _buildinfo = artifact_manager.generate_buildinfo()
            .context("Failed to generate buildinfo")?;

        let issues = artifact_manager.verify_integrity(artifacts)
            .context("Failed to verify artifact integrity")?;

        if !issues.is_empty() {
            for issue in &issues {
                warn!(target: "lmforge_release", issue = %issue, "integrity issue found");
            }
        }

        info!(target: "lmforge_release", "artifacts finalized");

        Ok(())
    }

    fn load_config(&self, cli: &Cli) -> Result<BuildConfig> {
        debug!(target: "lmforge_config", build_id = %self.build_id, "loading configuration");

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
            .with_user_config(&cli.config.clone().unwrap_or_default())?
            .with_cli_overrides(&partial_config);

        let config = loader.merge();

        debug!(target: "lmforge_config", "configuration loaded");
        Ok(config)
    }

    fn cleanup_build_dir(&self, config: &BuildConfig) -> Result<()> {
        let _logger = RuntimeLogger::new(&self.build_id.id);

        if config.workspace_dir.exists() {
            std::fs::remove_dir_all(&config.workspace_dir)?;
            debug!(target: "lmforge_cleanup", path = ?config.workspace_dir, "removed workspace directory");
        }

        if config.output_dir.exists() {
            for entry in std::fs::read_dir(&config.output_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().map_or(false, |ext| ext == "iso" || ext == "zst" || ext == "manifest" || path.file_name().map_or(false, |n| n == "SHA256SUMS")) {
                    std::fs::remove_file(&path)?;
                    debug!(target: "lmforge_cleanup", file = ?path, "removed old artifact");
                }
            }
        }

        Ok(())
    }

    fn create_platform(&self, ctx: &BuildContext) -> Result<Box<dyn Platform>> {
        stage_info!("workspace",
            platform_name = ctx.config.platform.name,
            "creating platform instance"
        );

        let platform: Box<dyn Platform> = Box::new(
            DebianPlatform::new(ctx.suite())
                .with_components(ctx.config.platform.components.clone())
        );

        Ok(platform)
    }

    fn build_pipeline(
        &self,
        _ctx: &mut BuildContext,
        platform: &dyn Platform,
        image_engine: &LiveBuildEngine,
    ) -> Result<Pipeline> {
        stage_info!("workspace", "building pipeline");

        let mut stages: Vec<Box<dyn Stage>> = Vec::new();

        stages.push(Box::new(WorkspaceStage {
            build_id: self.build_id.clone(),
        }));

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
            feature.register_stages(&mut stages)?;
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

struct WorkspaceStage {
    build_id: BuildIdStruct,
}

impl Stage for WorkspaceStage {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Initialize and prepare build workspace"
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("workspace");

        let start_time = Instant::now();

        stage_info!("workspace",
            root = ?ctx.workspace.root,
            rootfs = ?ctx.workspace.rootfs,
            cache = ?ctx.workspace.cache,
            temp = ?ctx.workspace.temp,
            "workspace directories ready"
        );

        let duration = start_time.elapsed();
        logger.log_stage_complete("workspace", duration);

        Ok(())
    }
}

struct BootstrapStage {
    platform_name: String,
    build_id: BuildIdStruct,
}

impl Stage for BootstrapStage {
    fn name(&self) -> &str {
        "bootstrap"
    }

    fn description(&self) -> &str {
        "Bootstrap base system using debootstrap/mmdebstrap"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["workspace"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        use crate::platform::debian::DebianPlatform;

        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("bootstrap");

        let start_time = Instant::now();

        let platform = DebianPlatform::new(ctx.suite())
            .with_components(ctx.config.platform.components.clone());

        platform.bootstrap(ctx)?;

        let duration = start_time.elapsed();
        logger.log_stage_complete("bootstrap", duration);

        Ok(())
    }
}

struct PackagesStage {
    build_id: BuildIdStruct,
}

impl Stage for PackagesStage {
    fn name(&self) -> &str {
        "packages"
    }

    fn description(&self) -> &str {
        "Configure and install packages into rootfs"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["bootstrap"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("packages");

        let start_time = Instant::now();

        let overlay_manager = OverlayManager::new_for_rootfs(&ctx.workspace.rootfs);
        let packages = overlay_manager.load_package_list()?;
        
        if !packages.is_empty() {
            stage_info!("packages",
                package_count = packages.len(),
                packages = ?packages,
                "custom packages configured"
            );
        } else {
            debug!(target: "lmforge_packages", "no custom package list found, using defaults from live-build");
        }

        let duration = start_time.elapsed();
        logger.log_stage_complete("packages", duration);

        Ok(())
    }
}

struct OverlayStage {
    build_id: BuildIdStruct,
}

impl Stage for OverlayStage {
    fn name(&self) -> &str {
        "overlay"
    }

    fn description(&self) -> &str {
        "Apply filesystem overlays and branding"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["packages"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("overlay");

        let start_time = Instant::now();

        stage_info!("overlay",
            overlay_dir = ?ctx.workspace.overlay,
            "applying overlays"
        );

        let overlay_manager = OverlayManager::new_for_rootfs(&ctx.workspace.rootfs);
        overlay_manager.apply_overlays(&ctx.workspace.rootfs)?;

        let duration = start_time.elapsed();
        logger.log_stage_complete("overlay", duration);

        Ok(())
    }
}

struct ImageStage {
    engine_name: String,
    build_id: BuildIdStruct,
}

impl Stage for ImageStage {
    fn name(&self) -> &str {
        "image"
    }

    fn description(&self) -> &str {
        "Generate ISO image using live-build"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["overlay"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("image");

        let start_time = Instant::now();

        stage_info!("image",
            engine = %self.engine_name,
            "generating image with live-build"
        );

        let image_engine = LiveBuildEngine::new();

        image_engine.prepare(ctx)?;
        let artifacts = image_engine.build(ctx)?;

        for artifact in &artifacts {
            stage_info!("image",
                filename = %artifact.filename(),
                kind = ?artifact.kind,
                size_bytes = artifact.size,
                "generated artifact"
            );
        }

        let duration = start_time.elapsed();
        logger.log_stage_complete("image", duration);

        Ok(())
    }
}

struct MetadataStage {
    build_id: BuildIdStruct,
}

impl Stage for MetadataStage {
    fn name(&self) -> &str {
        "metadata"
    }

    fn description(&self) -> &str {
        "Generate manifest, checksums, and build metadata"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["image"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        use crate::infra::checksum::ChecksumGenerator;

        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("metadata");

        let start_time = Instant::now();

        stage_info!("metadata", "generating manifest and checksums");

        let output_path = ctx.output_path();
        
        let checksums = {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                ChecksumGenerator::generate_checksums_for_directory(&output_path).await
            })?
        };

        if !checksums.is_empty() {
            let sha256_file = output_path.join("SHA256SUMS");
            ChecksumGenerator::write_checksum_file(&checksums, &sha256_file)?;
            
            stage_info!("metadata",
                file = ?sha256_file,
                count = checksums.len(),
                "written SHA256SUMS"
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

impl Stage for ReleaseStage {
    fn name(&self) -> &str {
        "release"
    }

    fn description(&self) -> &str {
        "Finalize release and collect all artifacts"
    }

    fn dependencies(&self) -> Vec<&str> {
        vec!["metadata"]
    }

    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        let logger = RuntimeLogger::new(&self.build_id.id);
        logger.log_stage_start("release");

        let start_time = Instant::now();

        stage_info!("release", "finalizing release");

        let artifacts = ctx.get_artifacts_sync();
        
        stage_info!("release",
            total_artifacts = artifacts.len(),
            "build complete"
        );

        let duration = start_time.elapsed();
        logger.log_stage_complete("release", duration);

        Ok(())
    }
}
