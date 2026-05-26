use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};

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
    pub checksum: String,
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
        Artifact {
            kind,
            path,
            checksum: String::new(),
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

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = checksum.into();
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
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }

    pub async fn compute_checksum(&mut self) -> Result<&str> {
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
        self.checksum = hex::encode(result);
        
        let metadata = tokio::fs::metadata(&self.path).await?;
        self.size = metadata.len();

        Ok(&self.checksum)
    }

    pub fn to_manifest_entry(&self) -> String {
        format!(
            "{}  {}  {}  {}",
            self.checksum,
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
