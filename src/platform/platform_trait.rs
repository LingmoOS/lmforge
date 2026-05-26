use async_trait::async_trait;
use anyhow::Result;
use crate::domain::context::BuildContext;

#[async_trait]
pub trait Platform: Send + Sync {
    fn name(&self) -> &str;

    async fn bootstrap(&self, ctx: &mut BuildContext) -> Result<()>;
    
    async fn install_packages(&self, ctx: &mut BuildContext, packages: &[&str]) -> Result<()>;

    async fn generate_repo_metadata(&self, ctx: &mut BuildContext) -> Result<()>;

    fn supported_architectures(&self) -> Vec<&str>;

    fn supported_suites(&self) -> Vec<&str>;

    fn package_manager_command(&self) -> &str;

    async fn validate_environment(&self) -> Result<()>;

    fn bootstrap_command(&self) -> &str;
}
