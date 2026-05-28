use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactKind {
    Iso,
    Rootfs,
    Manifest,
    Checksum,
    SourceArchive,
    BuildInfo,
    Squashfs,
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub filename: String,
    pub checksum: Option<String>,
    pub size: u64,
    pub metadata: ArtifactMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub arch: String,
    pub suite: String,
    pub version: String,
    pub build_time: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl Artifact {
    pub fn new(
        kind: ArtifactKind,
        path: PathBuf,
        arch: &str,
        suite: &str,
        version: &str,
    ) -> Self {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Artifact {
            kind,
            path,
            filename,
            checksum: None,
            size: 0,
            metadata: ArtifactMetadata {
                arch: arch.to_string(),
                suite: suite.to_string(),
                version: version.to_string(),
                build_time: chrono::Utc::now(),
                extra: None,
            },
        }
    }

    pub fn placeholder() -> Self {
        Artifact {
            kind: ArtifactKind::Manifest,
            path: PathBuf::new(),
            filename: "placeholder".to_string(),
            checksum: None,
            size: 0,
            metadata: ArtifactMetadata {
                arch: String::new(),
                suite: String::new(),
                version: String::new(),
                build_time: chrono::Utc::now(),
                extra: None,
            },
        }
    }

    pub fn is_placeholder(&self) -> bool {
        self.filename == "placeholder" && self.path.as_os_str().is_empty()
    }

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = Some(checksum.into());
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.metadata.extra = Some(extra);
        self
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub async fn compute_checksum(&mut self) -> Result<String> {
        use tokio::io::AsyncReadExt;
        
        let mut file = tokio::fs::File::open(&self.path).await?;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = vec![0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        self.checksum = Some(hex::encode(result));
        
        let metadata = tokio::fs::metadata(&self.path).await?;
        self.size = metadata.len();

        Ok(self.checksum.clone().unwrap_or_default())
    }

    pub fn to_manifest_entry(&self) -> String {
        format!(
            "{}  {}  {}  {}",
            self.checksum.as_deref().unwrap_or("none"),
            self.size,
            self.filename(),
            match &self.kind {
                ArtifactKind::Iso => "iso",
                ArtifactKind::Rootfs => "rootfs",
                ArtifactKind::Manifest => "manifest",
                ArtifactKind::Checksum => "checksum",
                ArtifactKind::SourceArchive => "source",
                ArtifactKind::BuildInfo => "buildinfo",
                ArtifactKind::Squashfs => "squashfs",
            }
        )
    }
}
