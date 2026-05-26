use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::context::BuildConfig;

const BUILTIN_CONFIG: &str = r#"
[arch]
default = "amd64"

[suite]
default = "bookworm"

[platform]
name = "debian"
components = ["main", "contrib", "non-free"]

[image]
engine = "livebuild"
iso_name = "lingmo-live.iso"
volume_id = "Lingmo Live"
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLayer {
    pub priority: u32,
    pub source: ConfigSource,
    pub config: PartialConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigSource {
    Builtin,
    Preset { name: String },
    Etc(String),
    User(PathBuf),
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialConfig {
    pub arch: Option<String>,
    pub suite: Option<String>,
    pub version: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub workspace_dir: Option<PathBuf>,
    pub features: Option<Vec<String>>,
    pub platform: Option<PartialPlatformConfig>,
    pub image: Option<PartialImageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialPlatformConfig {
    pub name: Option<String>,
    pub mirror: Option<String>,
    pub components: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialImageConfig {
    pub engine: Option<String>,
    pub iso_name: Option<String>,
    pub volume_id: Option<String>,
}

pub struct ConfigLoader {
    layers: Vec<ConfigLayer>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        ConfigLoader {
            layers: Vec::new(),
        }
    }

    pub fn with_builtin(mut self) -> Result<Self> {
        let config: PartialConfig = toml::from_str(BUILTIN_CONFIG)?;
        self.layers.push(ConfigLayer {
            priority: 0,
            source: ConfigSource::Builtin,
            config,
        });
        Ok(self)
    }

    pub fn with_preset(mut self, name: &str) -> Result<Self> {
        let preset_path = PathBuf::from("presets")
            .join(format!("{}.toml", name));
        
        if preset_path.exists() {
            let content = std::fs::read_to_string(&preset_path)?;
            let config: PartialConfig = toml::from_str(&content)?;
            self.layers.push(ConfigLayer {
                priority: 10,
                source: ConfigSource::Preset { name: name.to_string() },
                config,
            });
        }
        Ok(self)
    }

    pub fn with_etc_config(mut self) -> Result<Self> {
        let etc_path = PathBuf::from("/etc/lmforge/config.toml");
        if etc_path.exists() {
            let content = std::fs::read_to_string(&etc_path)?;
            let config: PartialConfig = toml::from_str(&content)?;
            self.layers.push(ConfigLayer {
                priority: 20,
                source: ConfigSource::Etc(etc_path.to_string_lossy().to_string()),
                config,
            });
        }
        Ok(self)
    }

    pub fn with_user_config(mut self, path: &PathBuf) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: PartialConfig = toml::from_str(&content)?;
            self.layers.push(ConfigLayer {
                priority: 30,
                source: ConfigSource::User(path.clone()),
                config,
            });
        }
        Ok(self)
    }

    pub fn with_cli_overrides(mut self, cli: &PartialConfig) -> Self {
        self.layers.push(ConfigLayer {
            priority: 40,
            source: ConfigSource::Cli,
            config: cli.clone(),
        });
        self
    }

    pub fn merge(self) -> BuildConfig {
        let mut base = BuildConfig::default();

        let mut sorted_layers = self.layers;
        sorted_layers.sort_by_key(|l| l.priority);

        for layer in sorted_layers {
            self.apply_partial(&mut base, &layer.config);
        }

        base
    }

    fn apply_partial(&self, base: &mut BuildConfig, partial: &PartialConfig) {
        if let Some(ref arch) = partial.arch {
            base.arch = arch.clone();
        }
        if let Some(ref suite) = partial.suite {
            base.suite = suite.clone();
        }
        if let Some(ref version) = partial.version {
            base.version = version.clone();
        }
        if let Some(ref output_dir) = partial.output_dir {
            base.output_dir = output_dir.clone();
        }
        if let Some(ref workspace_dir) = partial.workspace_dir {
            base.workspace_dir = workspace_dir.clone();
        }
        if let Some(ref features) = partial.features {
            base.features = features.clone();
        }

        if let Some(ref platform) = partial.platform {
            if let Some(ref name) = platform.name {
                base.platform.name = name.clone();
            }
            if let Some(ref mirror) = platform.mirror {
                base.platform.mirror = Some(mirror.clone());
            }
            if let Some(ref components) = platform.components {
                base.platform.components = components.clone();
            }
        }

        if let Some(ref image) = partial.image {
            if let Some(ref engine) = image.engine {
                base.image.engine = match engine.as_str() {
                    "native" => super::context::ImageEngineType::Native,
                    _ => super::context::ImageEngineType::LiveBuild,
                };
            }
            if let Some(ref iso_name) = image.iso_name {
                base.image.iso_name = iso_name.clone();
            }
            if let Some(ref volume_id) = image.volume_id {
                base.image.volume_id = volume_id.clone();
            }
        }
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}
