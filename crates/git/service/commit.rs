use crate::cache::{CachePolicy, TTL_COMMIT_BY_ID, TTL_LAST_COMMIT};
use crate::context::RepoContext;
use crate::models::commit::{CommitInfo, CommitMessage, FullCommitInfo};
use crate::parsers::commit::{parse_commit_record, parse_shortstat};
use crate::parsers::status::parse_name_status_z;
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
        let runner = self.ctx.runner.clone();
        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "last_commit",
                    ttl: TTL_LAST_COMMIT,
                },
                "head".to_string(),
                move || async move {
                    let record = runner
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
                },
            )
            .await
    }

    #[logger::logger]
    pub async fn commit_by_id(&self, hash: &str) -> Result<FullCommitInfo, String> {
        let hash = hash.to_string();
        let runner = self.ctx.runner.clone();
        self.ctx
            .cache
            .get_or_refresh(
                CachePolicy {
                    namespace: "commit_by_id",
                    ttl: TTL_COMMIT_BY_ID,
                },
                hash.clone(),
                move || async move {
                    let record = runner
                        .run_with_options(
                            &[
                                "show",
                                "-s",
                                "--format=%H%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%s%x1f%b",
                                &hash,
                            ],
                            GitRunOptions::default_read(),
                        )
                        .await?;

                    let commit_info = parse_commit_record(&record)?;

                    let stats_output = runner
                        .run_with_options(
                            &["show", "--shortstat", "--format=", &hash],
                            GitRunOptions::default_read(),
                        )
                        .await?;

                    let stats = parse_shortstat(&stats_output);
                    let files_output = runner
                        .run_with_options(
                            &["show", "--name-status", "-z", "--format=", &hash],
                            GitRunOptions::default_read(),
                        )
                        .await?;
                    let files = parse_name_status_z(files_output.as_bytes())?;

                    Ok(FullCommitInfo {
                        id: commit_info.id,
                        timestamp: commit_info.timestamp,
                        summary: commit_info.summary,
                        body: commit_info.body,
                        authors: commit_info.authors,
                        stats,
                        files,
                    })
                },
            )
            .await
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
            .await
            .map_err(|err| normalize_commit_error(&err))?;

        if output.contains("nothing to commit") {
            return Err("Nothing to commit (index matches HEAD)".to_string());
        }

        let commit_id = self
            .ctx
            .runner
            .run_with_options(&["rev-parse", "HEAD"], GitRunOptions::default_read())
            .await
            .unwrap_or_default();

        self.ctx.cache.invalidate_all();
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
                msg.push_str(&format!("Co-authored-by: {name} <{email}>\n"));
            }
        }

        msg
    }
}

fn normalize_commit_error(err: &str) -> String {
    if let Some(helper) = signing_helper_from_error(err) {
        return format!(
            "Git commit signing failed because `{helper}` could not be launched. Ensure the helper is installed and discoverable on PATH in the packaged app."
        );
    }

    err.trim().to_string()
}

fn signing_helper_from_error(err: &str) -> Option<String> {
    let lower = err.to_ascii_lowercase();
    if !lower.contains("failed to sign the data") {
        return None;
    }

    if let Some(helper) = parse_cannot_run_helper(err) {
        return Some(helper);
    }

    if lower.contains("gpg failed to sign the data") {
        return Some("gpg".to_string());
    }

    None
}

fn parse_cannot_run_helper(err: &str) -> Option<String> {
    let prefix = "cannot run ";
    let start = err.to_ascii_lowercase().find(prefix)?;
    let helper = &err[start + prefix.len()..];
    let helper = helper.split(':').next()?.trim();
    if helper.is_empty() {
        None
    } else {
        Some(helper.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_commit_error_reports_missing_helper() {
        let err = "error: cannot run gg: No such file or directory\nerror: gg failed to sign the data: (no gpg output)\nfatal: failed to write commit object";
        let normalized = normalize_commit_error(err);

        assert!(normalized.contains("gg"));
        assert!(normalized.contains("discoverable on PATH"));
    }

    #[test]
    fn normalize_commit_error_keeps_unrelated_errors() {
        let err = "fatal: not a git repository";
        let normalized = normalize_commit_error(err);

        assert_eq!(normalized, err);
    }
}
