use std::sync::Arc;

use crate::{
    AppState,
    context::RepoContext,
    service::{
        actions::ActionService, branch::BranchService, commit::CommitService, diff::DiffService,
        history::HistoryService, origin::OriginService, query::QueryService, stash::StashService,
    },
};

pub struct RepoServices {
    ctx: Arc<RepoContext>,
}

impl RepoServices {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        Ok(Self {
            ctx: Arc::new(RepoContext::new(repo_path)?),
        })
    }

    pub fn diff(&self) -> DiffService {
        DiffService::new(self.ctx.clone())
    }

    pub fn branch(&self) -> BranchService {
        BranchService::new(self.ctx.clone())
    }

    pub fn history(&self) -> HistoryService {
        HistoryService::new(self.ctx.clone())
    }

    pub fn origin(&self) -> OriginService {
        OriginService::new(self.ctx.clone())
    }

    pub fn commit(&self) -> CommitService {
        CommitService::new(self.ctx.clone())
    }

    pub fn action(&self) -> ActionService {
        ActionService::new(self.ctx.clone())
    }

    pub fn query(&self) -> QueryService {
        QueryService::new(self.ctx.clone())
    }

    pub fn stash(&self) -> StashService {
        StashService::new(self.ctx.clone())
    }
}

pub async fn get_services(state: tauri::State<'_, AppState>) -> Result<Arc<RepoServices>, String> {
    let lock = state.services.read().await;
    Ok(lock.clone().ok_or("Not initialized")?)
}
