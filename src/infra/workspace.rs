use std::path::PathBuf;
use anyhow::Result;
use chrono::Utc;
use tracing::{info, debug, warn};

#[derive(Clone)]
pub struct WorkspaceManager {
    base_dir: PathBuf,
    build_id: String,
    timestamp: String,
}

impl WorkspaceManager {
    pub fn new(base_dir: impl Into<PathBuf>, build_id: &str) -> Self {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        WorkspaceManager {
            base_dir: base_dir.into(),
            build_id: build_id.to_string(),
            timestamp,
        }
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub fn initialize(&self) -> Result<WorkspaceLayout> {
        let build_dir = self.base_dir.join(format!("build-{}-{}", self.timestamp, &self.build_id[..8]));
        
        info!(target: "lmforge_workspace", "initializing workspace at {:?}", build_dir);

        let layout = WorkspaceLayout {
            root: build_dir.clone(),
            config: build_dir.join("config"),
            cache: build_dir.join("cache"),
            artifacts: build_dir.join("artifacts"),
            logs: build_dir.join("logs"),
            runtime: build_dir.join("runtime"),
            temp: build_dir.join("temp"),
            rootfs: build_dir.join("rootfs"),
            output: build_dir.join("output"),
            overlay: build_dir.join("overlay"),
        };

        self.create_directories(&layout)?;
        self.create_build_info(&layout)?;

        info!(target: "lmforge_workspace", "workspace initialized successfully");
        debug!(target: "lmforge_workspace", root = ?layout.root, "workspace layout created");

        Ok(layout)
    }

    fn create_directories(&self, layout: &WorkspaceLayout) -> Result<()> {
        let dirs = [
            &layout.root,
            &layout.config,
            &layout.cache,
            &layout.artifacts,
            &layout.logs,
            &layout.runtime,
            &layout.temp,
            &layout.rootfs,
            &layout.output,
            &layout.overlay,
            &layout.config.join("package-lists"),
            &layout.config.join("includes.chroot"),
            &layout.config.join("hooks"),
            &layout.config.join("archives"),
            &layout.config.join("bootloaders"),
            &layout.logs.join("stages"),
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir)?;
            debug!(target: "lmforge_workspace", directory = ?dir, "created directory");
        }

        Ok(())
    }

    fn create_build_info(&self, layout: &WorkspaceLayout) -> Result<()> {
        let build_info_path = layout.root.join("build-info.json");
        
        let build_info = serde_json::json!({
            "build_id": self.build_id,
            "timestamp": self.timestamp,
            "workspace_root": layout.root.to_string_lossy(),
            "created_at": Utc::now().to_rfc3339(),
        });

        std::fs::write(&build_info_path, serde_json::to_string_pretty(&build_info)?)?;
        debug!(target: "lmforge_workspace", file = ?build_info_path, "wrote build info");

        Ok(())
    }

    pub fn cleanup_stale_workspaces(&self, max_age_days: u64) -> Result<Vec<PathBuf>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut cleaned = Vec::new();
        let max_age = chrono::Duration::days(max_age_days as i64);
        let cutoff = Utc::now() - max_age;

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("build-"))
                .unwrap_or(false)
            {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(modified) = metadata.modified() {
                        let modified_time = chrono::DateTime::<Utc>::from(modified);
                        
                        if modified_time < cutoff {
                            warn!(target: "lmforge_workspace", 
                                workspace = ?path, 
                                age_days = max_age_days,
                                "removing stale workspace"
                            );
                            
                            if std::fs::remove_dir_all(&path).is_ok() {
                                cleaned.push(path.clone());
                            }
                        }
                    }
                }
            }
        }

        if !cleaned.is_empty() {
            info!(target: "lmforge_workspace", count = cleaned.len(), "cleaned stale workspaces");
        }

        Ok(cleaned)
    }

    pub fn cleanup_temp(&self, layout: &WorkspaceLayout) -> Result<()> {
        if layout.temp.exists() {
            for entry in std::fs::read_dir(&layout.temp)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_file() {
                    std::fs::remove_file(&path)?;
                    debug!(target: "lmforge_workspace", file = ?path, "removed temp file");
                } else if path.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                    debug!(target: "lmforge_workspace", dir = ?path, "removed temp directory");
                }
            }
        }

        Ok(())
    }

    pub fn detect_interrupted_builds(&self) -> Result<Vec<InterruptedBuild>> {
        let mut interrupted = Vec::new();

        if !self.base_dir.exists() {
            return Ok(interrupted);
        }

        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("build-"))
                .unwrap_or(false)
            {
                let lock_file = path.join(".build.lock");
                let pid_file = path.join(".build.pid");
                
                if lock_file.exists() || pid_file.exists() {
                    interrupted.push(InterruptedBuild {
                        path: path.clone(),
                        has_lock_file: lock_file.exists(),
                        has_pid_file: pid_file.exists(),
                    });
                }
            }
        }

        Ok(interrupted)
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceLayout {
    pub root: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub artifacts: PathBuf,
    pub logs: PathBuf,
    pub runtime: PathBuf,
    pub temp: PathBuf,
    pub rootfs: PathBuf,
    pub output: PathBuf,
    pub overlay: PathBuf,
}

impl WorkspaceLayout {
    pub fn livebuild_config(&self) -> PathBuf {
        self.config.join("live-build")
    }

    pub fn stage_log(&self, stage_name: &str) -> PathBuf {
        self.logs.join("stages").join(format!("{}.log", stage_name))
    }

    pub fn artifact_output(&self, filename: &str) -> PathBuf {
        self.artifacts.join(filename)
    }
}

#[derive(Debug, Clone)]
pub struct InterruptedBuild {
    pub path: PathBuf,
    pub has_lock_file: bool,
    pub has_pid_file: bool,
}
