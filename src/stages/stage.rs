use anyhow::Result;
use async_trait::async_trait;
use crate::domain::context::BuildContext;

#[async_trait]
pub trait Stage: Send + Sync {
    fn name(&self) -> &str;
    
    async fn run(&self, ctx: &mut BuildContext) -> Result<()>;
    
    fn description(&self) -> &str {
        ""
    }

    fn dependencies(&self) -> Vec<&str> {
        vec![]
    }
}

#[async_trait]
impl Stage for Box<dyn Stage> {
    fn name(&self) -> &str {
        (**self).name()
    }
    
    async fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        (**self).run(ctx).await
    }
    
    fn description(&self) -> &str {
        (**self).description()
    }

    fn dependencies(&self) -> Vec<&str> {
        (**self).dependencies()
    }
}
