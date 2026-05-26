use async_trait::async_trait;
use anyhow::Result;
use crate::domain::context::BuildContext;
use crate::domain::artifact::Artifact;

#[async_trait]
pub trait ImageEngine: Send + Sync {
    fn name(&self) -> &str;

    async fn prepare(&self, ctx: &mut BuildContext) -> Result<()>;

    async fn build(&self, ctx: &mut BuildContext) -> Result<Vec<Artifact>>;

    async fn cleanup(&self, ctx: &mut BuildContext) -> Result<()>;

    fn supported_formats(&self) -> Vec<&str>;
}
