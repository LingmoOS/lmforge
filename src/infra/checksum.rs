use std::path::PathBuf;
use anyhow::Result;
use tracing::info;
use sha2::{Sha256, Digest};
use hex;

pub struct ChecksumGenerator;

impl ChecksumGenerator {
    pub async fn sha256_file(path: &PathBuf) -> Result<String> {
        let content = tokio::fs::read(path).await?;
        
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let result = hasher.finalize();

        Ok(hex::encode(result))
    }

    pub async fn generate_checksums_for_directory(dir: &PathBuf) -> Result<Vec<(String, String)>> {
        let mut checksums = Vec::new();

        if !dir.exists() {
            return Ok(checksums);
        }

        let mut entries = tokio::fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() {
                let filename = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let checksum = Self::sha256_file(&path).await?;
                
                info!("Generated SHA256 for {}: {}", filename, &checksum[..16]);
                checksums.push((filename, checksum));
            }
        }

        Ok(checksums)
    }

    pub fn write_checksum_file(checksums: &[(String, String)], output: &PathBuf) -> Result<()> {
        let mut content = String::new();
        
        for (filename, checksum) in checksums {
            content.push_str(&format!("{}  {}\n", checksum, filename));
        }

        std::fs::write(output, content)?;

        info!("Checksum file written to {:?}", output);
        Ok(())
    }

    pub async fn verify_checksum(file: &PathBuf, expected: &str) -> Result<bool> {
        let actual = Self::sha256_file(file).await?;
        
        Ok(actual == expected)
    }
}
