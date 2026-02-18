use crate::{
    context::RepoContext, models::origin::RepositoryOrigin, parsers::origin::parse_remote_url,
    runner::GitRunOptions,
};
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
        let url = self
            .ctx
            .runner
            .run_with_options(
                &["remote", "get-url", "origin"],
                GitRunOptions::default_read(),
            )
            .await
            .map_err(|_| "No origin remote found".to_string())?;

        let (protocol, host, owner, repo_name, provider) = parse_remote_url(&url);

        Ok(RepositoryOrigin {
            remote_name: "origin".into(),
            remote_url: url,
            host,
            provider,
            owner,
            repo: repo_name,
            protocol,
        })
    }
}
