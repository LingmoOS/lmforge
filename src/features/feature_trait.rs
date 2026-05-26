use async_trait::async_trait;
use anyhow::Result;
use crate::domain::context::BuildContext;
use crate::stages::stage::Stage;

#[async_trait]
pub trait Feature: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    fn dependencies(&self) -> Vec<&str> {
        vec![]
    }

    fn conflicts_with(&self) -> Vec<&str> {
        vec![]
    }

    async fn register_stages(&self, pipeline: &mut Vec<Box<dyn Stage>>) -> Result<()>;

    async fn prepare_overlay(&self, ctx: &mut BuildContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
