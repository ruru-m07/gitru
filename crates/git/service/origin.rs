use crate::{context::RepoContext, models::origin::RepositoryOrigin, service::query::QueryService};
use std::sync::Arc;

pub struct OriginService {
    ctx: Arc<RepoContext>,
}

impl OriginService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn repository_origin(&self) -> Result<RepositoryOrigin, String> {
        QueryService::new(self.ctx.clone())
            .repository_origin()
            .await
    }
}
