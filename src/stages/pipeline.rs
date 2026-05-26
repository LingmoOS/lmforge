use std::collections::{HashMap, HashSet};
use anyhow::{Result, bail};
use tracing::{info, warn, error};

use super::stage::Stage;
use crate::domain::context::BuildContext;

pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
    registry: HashMap<String, usize>,
}

impl Pipeline {
    pub fn new() -> Self {
        Pipeline {
            stages: Vec::new(),
            registry: HashMap::new(),
        }
    }

    pub fn register(mut self, stage: Box<dyn Stage>) -> Result<Self> {
        let name = stage.name().to_string();
        
        if self.registry.contains_key(&name) {
            bail!("Stage '{}' already registered", name);
        }

        let index = self.stages.len();
        self.registry.insert(name.clone(), index);
        self.stages.push(stage);

        info!("Registered stage: {}", name);
        Ok(self)
    }

    pub fn with_stages(stages: Vec<Box<dyn Stage>>) -> Result<Self> {
        let mut pipeline = Self::new();
        for stage in stages {
            pipeline = pipeline.register(stage)?;
        }
        Ok(pipeline)
    }

    fn validate_order(&self) -> Result<()> {
        for (i, stage) in self.stages.iter().enumerate() {
            for dep in stage.dependencies() {
                if let Some(&dep_index) = self.registry.get(dep) {
                    if dep_index >= i {
                        bail!(
                            "Stage '{}' depends on '{}' which is not yet executed",
                            stage.name(),
                            dep
                        );
                    }
                } else {
                    bail!(
                        "Stage '{}' depends on unknown stage '{}'",
                        stage.name(),
                        dep
                    );
                }
            }
        }
        Ok(())
    }

    pub async fn execute(&self, ctx: &mut BuildContext) -> Result<Vec<String>> {
        self.validate_order()?;
        
        let mut completed = Vec::new();
        ctx.runtime_state.start_time = Some(chrono::Utc::now());

        info!("Pipeline starting with {} stages", self.stages.len());

        for stage in &self.stages {
            let stage_name = stage.name().to_string();
            
            ctx.set_current_stage(&stage_name);
            ctx.log(
                crate::domain::context::LogLevel::Info,
                &stage_name,
                &format!("Starting stage: {}", stage.description()),
            )
            .await;

            info!("Executing stage: {} - {}", stage_name, stage.description());

            match stage.run(ctx).await {
                Ok(_) => {
                    ctx.complete_stage(&stage_name);
                    completed.push(stage_name.clone());
                    
                    ctx.log(
                        crate::domain::context::LogLevel::Info,
                        &stage_name,
                        "Stage completed successfully",
                    )
                    .await;
                    
                    info!("Stage '{}' completed successfully", stage_name);
                }
                Err(e) => {
                    let error_msg = format!("Stage '{}' failed: {}", stage_name, e);
                    ctx.record_error(&error_msg);
                    
                    ctx.log(
                        crate::domain::context::LogLevel::Error,
                        &stage_name,
                        &error_msg,
                    )
                    .await;
                    
                    error!("{}", error_msg);
                    
                    return Err(anyhow::anyhow!(error_msg));
                }
            }
        }

        ctx.runtime_state.end_time = Some(chrono::Utc::now());
        info!("Pipeline completed: {}/{} stages successful", completed.len(), self.stages.len());

        Ok(completed)
    }

    pub fn stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.name()).collect()
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}
