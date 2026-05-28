use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};

use crate::domain::artifact::{Artifact, ArtifactKind};
use crate::infra::workspace::WorkspaceLayout;
use crate::infra::checksum::ChecksumGenerator;

pub struct ArtifactManager {
    workspace: WorkspaceLayout,
    build_id: String,
}

impl ArtifactManager {
    pub fn new(workspace: &WorkspaceLayout, build_id: &str) -> Self {
        ArtifactManager {
            workspace: workspace.clone(),
            build_id: build_id.to_string(),
        }
    }

    pub fn collect_iso(&self, source: &Path, filename: &str) -> Result<Artifact> {
        info!(
            target: "lmforge_artifact",
            artifact = %filename,
            source = ?source,
            "collecting ISO artifact"
        );

        let dest = self.workspace.artifact_output(filename);
        
        if !source.exists() {
            warn!(
                target: "lmforge_artifact",
                source = ?source,
                "ISO file not found at expected location"
            );
            
            self.search_for_iso(source.parent().unwrap_or(source), filename)
                .or_else(|_| Err(anyhow::anyhow!("ISO file not found: {:?}", source)))
        } else {
            self.copy_and_register(source, &dest, ArtifactKind::Iso, filename)
        }
    }

    pub fn collect_squashfs(&self, source: &Path) -> Result<Artifact> {
        let filename = "filesystem.squashfs";
        
        info!(
            target: "lmforge_artifact",
            artifact = %filename,
            source = ?source,
            "collecting SquashFS artifact"
        );

        let dest = self.workspace.artifact_output(filename);

        if !source.exists() {
            warn!(
                target: "lmforge_artifact",
                source = ?source,
                "SquashFS file not found"
            );
            return Ok(Artifact::placeholder());
        }

        self.copy_and_register(source, &dest, ArtifactKind::Squashfs, filename)
    }

    pub fn collect_all(&self, lb_output_dir: &Path, iso_name: &str) -> Result<Vec<Artifact>> {
        info!(
            target: "lmforge_artifact",
            output_dir = ?lb_output_dir,
            "collecting all artifacts from live-build output"
        );

        let mut artifacts = Vec::new();

        let iso_filename = format!("{}.iso", iso_name);
        let iso_source = lb_output_dir.join(&iso_filename);

        match self.collect_iso(&iso_source, &iso_filename) {
            Ok(artifact) => artifacts.push(artifact),
            Err(e) => warn!(target: "lmforge_artifact", error = %e, "failed to collect ISO"),
        }

        let squashfs_path = lb_output_dir.join("binary/live/filesystem.squashfs");
        match self.collect_squashfs(&squashfs_path) {
            Ok(artifact) => {
                if !artifact.is_placeholder() {
                    artifacts.push(artifact);
                }
            }
            Err(e) => warn!(target: "lmforge_artifact", error = %e, "failed to collect SquashFS"),
        }

        info!(
            target: "lmforge_artifact",
            count = artifacts.len(),
            "collected {} artifacts",
            artifacts.len()
        );

        Ok(artifacts)
    }

    fn copy_and_register(
        &self,
        source: &Path,
        dest: &Path,
        kind: ArtifactKind,
        filename: &str,
    ) -> Result<Artifact> {
        std::fs::create_dir_all(dest.parent().context("Invalid destination path")?)?;

        let rt = tokio::runtime::Runtime::new()?;
        
        rt.block_on(async move {
            tokio::fs::copy(source, dest).await
        })?;

        let mut artifact = Artifact::new(kind, dest.to_path_buf(), "", "", "");
        artifact.filename = filename.to_string();

        {
            let dest_clone = dest.to_path_buf();
            let checksum_result = {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    ChecksumGenerator::sha256_file(&dest_clone).await
                })
            };

            match checksum_result {
                Ok(checksum) => {
                    artifact.checksum = Some(checksum.clone());
                    
                    debug!(
                        target: "lmforge_artifact",
                        artifact = %filename,
                        checksum = %checksum,
                        "artifact collected with checksum"
                    );
                }
                Err(e) => {
                    warn!(
                        target: "lmforge_artifact",
                        artifact = %filename,
                        error = %e,
                        "failed to compute checksum"
                    );
                }
            }
        }

        Ok(artifact)
    }

    fn search_for_iso(&self, search_dir: &Path, iso_name: &str) -> Result<Artifact> {
        info!(
            target: "lmforge_artifact",
            search_dir = ?search_dir,
            name = %iso_name,
            "searching for ISO in directory tree"
        );

        for entry in walkdir::WalkDir::new(search_dir).max_depth(3) {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "iso").unwrap_or(false) {
                let found_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown.iso");

                if found_name.contains(iso_name.trim_end_matches(".iso")) || path.file_name().map(|n| n == iso_name).unwrap_or(false) {
                    info!(
                        target: "lmforge_artifact",
                        found = ?path,
                        "found ISO file"
                    );

                    return self.collect_iso(path, found_name);
                }
            }
        }

        Err(anyhow::anyhow!("ISO not found in {:?}", search_dir))
    }

    pub fn generate_checksums_file(&self, artifacts: &[Artifact]) -> Result<PathBuf> {
        let checksums_path = self.workspace.artifacts.join("SHA256SUMS");
        
        info!(
            target: "lmforge_artifact",
            file = ?checksums_path,
            count = artifacts.len(),
            "generating SHA256SUMS file"
        );

        let mut content = String::new();

        for artifact in artifacts {
            if let Some(checksum) = &artifact.checksum {
                content.push_str(&format!("{}  {}\n", checksum, artifact.filename));
            } else if artifact.path.exists() {
                let path_clone = artifact.path.clone();
                match {
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(async {
                        ChecksumGenerator::sha256_file(&path_clone).await
                    })
                } {
                    Ok(checksum) => {
                        content.push_str(&format!("{}  {}\n", checksum, artifact.filename));
                        debug!(
                            target: "lmforge_artifact",
                            artifact = %artifact.filename,
                            checksum = %checksum,
                            "computed checksum for manifest"
                        );
                    }
                    Err(e) => {
                        warn!(
                            target: "lmforge_artifact",
                            artifact = %artifact.filename,
                            error = %e,
                            "failed to compute checksum"
                        );
                    }
                }
            }
        }

        std::fs::write(&checksums_path, content)?;

        info!(
            target: "lmforge_artifact",
            file = ?checksums_path,
            "SHA256SUMS file generated"
        );

        Ok(checksums_path)
    }

    pub fn generate_build_manifest(
        &self,
        artifacts: &[Artifact],
        config: &serde_json::Value,
    ) -> Result<PathBuf> {
        let manifest_path = self.workspace.artifacts.join("build-manifest.json");

        info!(
            target: "lmforge_artifact",
            file = ?manifest_path,
            "generating build manifest"
        );

        let manifest = serde_json::json!({
            "build_id": self.build_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "artifacts": artifacts.iter().map(|a| {
                serde_json::json!({
                    "filename": a.filename,
                    "kind": format!("{:?}", a.kind),
                    "size_bytes": a.path.metadata().ok().map(|m| m.len()).unwrap_or(0),
                    "checksum_sha256": a.checksum,
                    "path": a.path.to_string_lossy(),
                })
            }).collect::<Vec<_>>(),
            "config": config,
        });

        std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

        info!(
            target: "lmforge_artifact",
            file = ?manifest_path,
            "build manifest generated"
        );

        Ok(manifest_path)
    }

    pub fn generate_buildinfo(&self) -> Result<String> {
        let buildinfo_content = format!(
            r#"Build-ID: {build_id}
Format: 1.0
Source: lmforge
Architecture: auto
Binary-Architecture: auto
Timestamp: {timestamp}
Environment:
  UNAME-MACHINE={uname}
"#,
            build_id = self.build_id,
            timestamp = chrono::Utc::now().to_rfc3339(),
            uname = get_uname_machine(),
        );

        let buildinfo_path = self.workspace.artifacts.join("buildinfo");

        std::fs::write(&buildinfo_path, buildinfo_content)?;

        info!(
            target: "lmforge_artifact",
            file = ?buildinfo_path,
            "buildinfo generated"
        );

        Ok(buildinfo_path.to_string_lossy().to_string())
    }

    pub fn verify_integrity(&self, artifacts: &[Artifact]) -> Result<Vec<String>> {
        info!(
            target: "lmforge_artifact",
            count = artifacts.len(),
            "verifying artifact integrity"
        );

        let mut issues = Vec::new();

        for artifact in artifacts {
            if !artifact.path.exists() {
                let msg = format!("Missing artifact: {}", artifact.filename);
                error!(target: "lmforge_artifact", artifact = %artifact.filename, "artifact missing");
                issues.push(msg);
                continue;
            }

            if let Some(expected_checksum) = &artifact.checksum {
                let path_clone = artifact.path.clone();
                match {
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(async {
                        ChecksumGenerator::sha256_file(&path_clone).await
                    })
                } {
                    Ok(actual_checksum) => {
                        if actual_checksum != *expected_checksum {
                            let msg = format!(
                                "Checksum mismatch for {}: expected {}, got {}",
                                artifact.filename,
                                expected_checksum,
                                actual_checksum
                            );
                            error!(
                                target: "lmforge_artifact",
                                artifact = %artifact.filename,
                                expected = %expected_checksum,
                                actual = %actual_checksum,
                                "checksum mismatch detected"
                            );
                            issues.push(msg);
                        } else {
                            debug!(
                                target: "lmforge_artifact",
                                artifact = %artifact.filename,
                                "integrity verified"
                            );
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to compute checksum for {}: {}", artifact.filename, e);
                        warn!(target: "lmforge_artifact", error = %e, artifact = %artifact.filename, "verification failed");
                        issues.push(msg);
                    }
                }
            }
        }

        if issues.is_empty() {
            info!(target: "lmforge_artifact", "all artifacts passed integrity check");
        } else {
            error!(
                target: "lmforge_artifact",
                count = issues.len(),
                "{} integrity issues found",
                issues.len()
            );
        }

        Ok(issues)
    }
}

fn get_uname_machine() -> String {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("uname")
            .arg("-m")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }
    
    #[cfg(not(unix))]
    {
        "unknown".to_string()
    }
}
