use anyhow::Result;
use crate::domain::context::BuildContext;

pub trait Stage: Send + Sync {
    fn name(&self) -> &str;
    
    fn run(&self, ctx: &mut BuildContext) -> Result<()>;
    
    fn description(&self) -> &str {
        ""
    }

    fn dependencies(&self) -> Vec<&str> {
        vec![]
    }
}

impl Stage for Box<dyn Stage> {
    fn name(&self) -> &str {
        (**self).name()
    }
    
    fn run(&self, ctx: &mut BuildContext) -> Result<()> {
        (**self).run(ctx)
    }
    
    fn description(&self) -> &str {
        (**self).description()
    }

    fn dependencies(&self) -> Vec<&str> {
        (**self).dependencies()
    }
}
