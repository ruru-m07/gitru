use crate::cache::{CachePolicy, TTL_HISTORY};
use crate::context::RepoContext;
use crate::models::commit::CommitInfo;
use crate::models::graph::HistoryQuery;
use crate::models::history::HistoryGraphResponse;
use crate::parsers::commit::COMMIT_STANDARD_FORMAT;
use crate::parsers::history::parse_history_records;
use crate::runner::GitRunOptions;
use crate::service::graph as graph_service;
use std::sync::Arc;
use std::time::Duration;

pub struct HistoryService {
    ctx: Arc<RepoContext>,
}

impl HistoryService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn history(&self, skip: usize, limit: usize) -> Result<Vec<CommitInfo>, String> {
        let skip_str = skip.to_string();
        let limit_str = limit.to_string();
        let runner = self.ctx.runner.clone();
        let key = format!("skip={skip}:limit={limit}");

        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "history",
                    ttl: TTL_HISTORY,
                },
                key,
                move || async move {
                    let output = runner
                        .run_with_options(
                            &[
                                "log",
                                "--format",
                                COMMIT_STANDARD_FORMAT,
                                "--skip",
                                &skip_str,
                                "-n",
                                &limit_str,
                            ],
                            GitRunOptions::default_read().with_timeout(Duration::from_secs(60)),
                        )
                        .await?;

                    parse_history_records(&output)
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn history_graph(&self, query: HistoryQuery) -> Result<HistoryGraphResponse, String> {
        graph_service::history_graph(&self.ctx.repo_path, &query)
    }
}
