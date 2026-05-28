use anyhow::Result;
use crate::domain::context::BuildContext;
use crate::domain::artifact::Artifact;

pub trait ImageEngine: Send + Sync {
    fn name(&self) -> &str;

    fn prepare(&self, ctx: &mut BuildContext) -> Result<()>;

    fn build(&self, ctx: &mut BuildContext) -> Result<Vec<Artifact>>;

    fn cleanup(&self, ctx: &mut BuildContext) -> Result<()>;

    fn supported_formats(&self) -> Vec<&str>;
}
