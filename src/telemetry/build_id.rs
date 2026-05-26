use std::path::PathBuf;
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BuildId {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub short_id: String,
}

impl BuildId {
    pub fn new() -> Self {
        let timestamp = Utc::now();
        let date_str = timestamp.format("%Y%m%d").to_string();
        let uuid_short = Uuid::new_v4().to_string()[..8].to_string();
        
        let id = format!("build-{}-{}", date_str, uuid_short);
        
        BuildId {
            id: id.clone(),
            timestamp,
            short_id: uuid_short,
        }
    }

    pub fn from_string(id: String) -> Self {
        let short_id = id.split('-').last().unwrap_or(&id).to_string();
        
        BuildId {
            id: id.clone(),
            timestamp: Utc::now(),
            short_id,
        }
    }

    pub fn output_dir(&self, base: &PathBuf) -> PathBuf {
        base.join(&self.id)
    }

    pub fn logs_dir(&self, base: &PathBuf) -> PathBuf {
        self.output_dir(base).join("logs")
    }

    pub fn artifacts_dir(&self, base: &PathBuf) -> PathBuf {
        self.output_dir(base).join("artifacts")
    }

    pub fn metadata_dir(&self, base: &PathBuf) -> PathBuf {
        self.output_dir(base).join("metadata")
    }

    pub fn temp_dir(&self, base: &PathBuf) -> PathBuf {
        self.output_dir(base).join("temp")
    }

    pub fn cache_dir(&self, base: &PathBuf) -> PathBuf {
        self.output_dir(base).join("cache")
    }

    pub fn create_directories(&self, base: &PathBuf) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.output_dir(base))?;
        std::fs::create_dir_all(self.logs_dir(base))?;
        std::fs::create_dir_all(self.artifacts_dir(base))?;
        std::fs::create_dir_all(self.metadata_dir(base))?;
        std::fs::create_dir_all(self.temp_dir(base))?;
        std::fs::create_dir_all(self.cache_dir(base))?;
        
        Ok(())
    }
}

impl Default for BuildId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BuildId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}
