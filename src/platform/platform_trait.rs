use anyhow::Result;
use crate::domain::context::BuildContext;

pub trait Platform: Send + Sync {
    fn name(&self) -> &str;

    fn bootstrap(&self, ctx: &mut BuildContext) -> Result<()>;
    
    fn install_packages(&self, ctx: &mut BuildContext, packages: &[&str]) -> Result<()>;

    fn generate_repo_metadata(&self, ctx: &mut BuildContext) -> Result<()>;

    fn supported_architectures(&self) -> Vec<&str>;

    fn supported_suites(&self) -> Vec<&str>;

    fn package_manager_command(&self) -> &str;

    fn validate_environment(&self) -> Result<()>;

    fn bootstrap_command(&self) -> &str;
}
