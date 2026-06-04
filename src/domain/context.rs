use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use anyhow::Result;
use serde::{Serialize, Deserialize};

use crate::domain::artifact::Artifact;
use crate::domain::config::RepositoryDefinition;
use crate::infra::workspace::WorkspaceLayout;
use crate::runtime::MountManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub arch: String,
    pub suite: String,
    pub version: String,
    pub output_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub features: Vec<String>,
    pub platform: PlatformConfig,
    pub image: ImageConfig,
    pub repositories: Vec<RepositoryDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub name: String,
    pub mirror: Option<String>,
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub engine: ImageEngineType,
    pub iso_name: String,
    pub volume_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageEngineType {
    LiveBuild,
    Native,
}

impl Default for BuildConfig {
    fn default() -> Self {
        BuildConfig {
            arch: "amd64".to_string(),
            suite: "trixie".to_string(),
            version: "1.0.0".to_string(),
            output_dir: PathBuf::from("./output"),
            workspace_dir: PathBuf::from("./workspace"),
            features: vec![],
            platform: PlatformConfig {
                name: "debian".to_string(),
                mirror: None,
                components: vec!["main".to_string(), "contrib".to_string(), "non-free".to_string(), "non-free-firmware".to_string()],
            },
            image: ImageConfig {
                engine: ImageEngineType::LiveBuild,
                iso_name: "lingmo-live.iso".to_string(),
                volume_id: "Lingmo Live".to_string(),
            },
            repositories: vec![],
        }
    }
}

pub struct BuildContext {
    pub config: Arc<BuildConfig>,
    pub workspace: Workspace,
    pub workspace_layout: Option<WorkspaceLayout>,
    pub artifacts: Arc<RwLock<Vec<Artifact>>>,
    pub runtime_state: RuntimeState,
    pub logs: Arc<RwLock<Vec<LogEntry>>>,
    pub mount_manager: Option<Arc<MountManager>>,
    pub log_level: crate::runtime::log_stream::LogLevel,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub rootfs: PathBuf,
    pub chroot: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub overlay: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeState {
    pub current_stage: Option<String>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub stages_completed: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl BuildContext {
    pub fn new(config: BuildConfig, log_level: crate::runtime::log_stream::LogLevel) -> Result<Self> {
        let config = Arc::new(config);
        let workspace = Self::create_workspace(&config)?;
        
        Ok(BuildContext {
            config,
            workspace,
            workspace_layout: None,
            artifacts: Arc::new(RwLock::new(Vec::new())),
            runtime_state: RuntimeState::default(),
            logs: Arc::new(RwLock::new(Vec::new())),
            mount_manager: None,
            log_level,
        })
    }

    fn create_workspace(config: &BuildConfig) -> Result<Workspace> {
        let root = &config.workspace_dir;
        
        let workspace = Workspace {
            root: root.clone(),
            rootfs: root.join("rootfs"),
            chroot: root.join("chroot"),
            cache: root.join("cache"),
            temp: root.join("temp"),
            overlay: root.join("overlay"),
        };

        for dir in [
            &workspace.root,
            &workspace.rootfs,
            &workspace.chroot,
            &workspace.cache,
            &workspace.temp,
            &workspace.overlay,
        ] {
            std::fs::create_dir_all(dir)?;
        }

        Ok(workspace)
    }

    pub fn register_artifact(&self, artifact: Artifact) {
        let mut artifacts = self.artifacts.write().unwrap();
        artifacts.push(artifact);
    }

    pub fn get_artifacts_sync(&self) -> Vec<Artifact> {
        let artifacts = self.artifacts.read().unwrap();
        artifacts.clone()
    }

    pub fn log(&self, level: LogLevel, stage: &str, message: &str) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            stage: stage.to_string(),
            message: message.to_string(),
        };
        let mut logs = self.logs.write().unwrap();
        logs.push(entry);
    }

    pub fn set_current_stage(&mut self, stage: &str) {
        self.runtime_state.current_stage = Some(stage.to_string());
    }

    pub fn complete_stage(&mut self, stage: &str) {
        self.runtime_state.stages_completed.push(stage.to_string());
        if self.runtime_state.current_stage.as_deref() == Some(stage) {
            self.runtime_state.current_stage = None;
        }
    }

    pub fn record_error(&mut self, error: &str) {
        self.runtime_state.errors.push(error.to_string());
    }

    pub fn output_path(&self) -> &PathBuf {
        &self.config.output_dir
    }

    pub fn ensure_output_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config.output_dir)?;
        Ok(())
    }

    pub fn arch(&self) -> &str {
        &self.config.arch
    }

    pub fn suite(&self) -> &str {
        &self.config.suite
    }

    pub fn version(&self) -> &str {
        &self.config.version
    }

    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.config.features.contains(&feature.to_string())
    }

    pub fn cleanup_workspace(&self) -> Result<()> {
        if self.workspace.root.exists() {
            std::fs::remove_dir_all(&self.workspace.root)?;
        }
        Ok(())
    }
}
