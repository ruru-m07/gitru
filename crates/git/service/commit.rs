use crate::context::RepoContext;
use crate::models::commit::{CommitInfo, CommitMessage, FullCommitInfo};
use crate::parsers::commit::{parse_commit_record, parse_shortstat};
use crate::runner::GitRunOptions;
use std::sync::Arc;

pub struct CommitService {
    ctx: Arc<RepoContext>,
}

impl CommitService {
    pub fn new(ctx: Arc<RepoContext>) -> Self {
        Self { ctx }
    }

    #[logger::logger]
    pub async fn last_commit(&self) -> Result<CommitInfo, String> {
        let record = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "log",
                    "-1",
                    "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%s%x1f%b",
                ],
                GitRunOptions::default_read(),
            )
            .await?;

        parse_commit_record(&record)
    }

    #[logger::logger]
    pub async fn commit_by_id(&self, hash: &str) -> Result<FullCommitInfo, String> {
        let record = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "show",
                    "-s",
                    "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%s%x1f%b",
                    hash,
                ],
                GitRunOptions::default_read(),
            )
            .await?;

        let commit_info = parse_commit_record(&record)?;

        let stats_output = self
            .ctx
            .runner
            .run_with_options(
                &["show", "--shortstat", "--format=", hash],
                GitRunOptions::default_read(),
            )
            .await?;

        let stats = parse_shortstat(&stats_output);

        Ok(FullCommitInfo {
            id: commit_info.id,
            timestamp: commit_info.timestamp,
            summary: commit_info.summary,
            body: commit_info.body,
            authors: commit_info.authors,
            stats,
        })
    }

    #[logger::logger]
    pub async fn create_commit(
        &self,
        commit_meta: &CommitMessage,
        allow_empty: bool,
    ) -> Result<String, String> {
        let is_bare = self
            .ctx
            .runner
            .run_with_options(
                &["rev-parse", "--is-bare-repository"],
                GitRunOptions::default_read(),
            )
            .await
            .unwrap_or_default();

        if is_bare == "true" {
            return Err("Cannot commit in a bare repository".to_string());
        }

        let message = self.build_commit_message(commit_meta);

        let mut args = vec!["commit", "-F", "-"];
        if allow_empty {
            args.push("--allow-empty");
        }

        let output = self
            .ctx
            .runner
            .run_with_input(
                &args,
                &message,
                GitRunOptions::default_read().allow_exit_codes(&[1]),
            )
            .await?;

        if output.contains("nothing to commit") {
            return Err("Nothing to commit (index matches HEAD)".to_string());
        }

        let commit_id = self
            .ctx
            .runner
            .run_with_options(&["rev-parse", "HEAD"], GitRunOptions::default_read())
            .await
            .unwrap_or_default();

        Ok(commit_id)
    }

    fn build_commit_message(&self, commit_meta: &CommitMessage) -> String {
        let mut msg = String::new();

        msg.push_str(commit_meta.title.trim());
        msg.push('\n');

        if let Some(desc) = commit_meta.description.as_deref() {
            msg.push('\n');
            msg.push_str(desc.trim());
            msg.push('\n');
        }

        if !commit_meta.co_authors.is_empty() {
            msg.push('\n');
            for (name, email) in &commit_meta.co_authors {
                msg.push_str(&format!("Co-authored-by: {} <{}>\n", name, email));
            }
        }

        msg
    }
}
