use crate::cache::{CachePolicy, TTL_COMMIT_BY_ID, TTL_LAST_COMMIT};
use crate::context::RepoContext;
use crate::models::commit::{Author, CommitInfo, CommitMessage, FullCommitInfo};
use crate::models::operation::{RepoOperation, RepoOperationKind};
use crate::models::rebase::RebasePauseReason;
use crate::parsers::commit::{COMMIT_STANDARD_FORMAT, parse_commit_record, parse_shortstat};
use crate::parsers::history::parse_history_records;
use crate::parsers::status::parse_name_status_z;
use crate::runner::GitRunOptions;
use crate::service::operation::OperationService;
use std::collections::HashSet;
use std::sync::Arc;

const RECENT_COMMIT_AUTHOR_LIMIT: &str = "100";

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
        amend: bool,
        expected_head: Option<&str>,
    ) -> Result<String, String> {
        validate_commit_request(commit_meta, allow_empty, amend, expected_head)?;

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

        // Keep the operation snapshot, amend lease, commit, and resulting HEAD read
        // in one Gitru-internal critical section. Hooks and signing helpers still run
        // normally inside `git commit`; arbitrary external Git processes are not gated.
        let mut transaction = self.ctx.runner.transaction().await?;
        let operation = OperationService::new(self.ctx.clone()).get_repo_operation()?;
        validate_commit_operation(&operation, allow_empty, amend)?;

        if amend {
            let expected_head = expected_head
                .map(str::trim)
                .filter(|head| !head.is_empty())
                .ok_or_else(|| "Expected HEAD is required when amending a commit".to_string())?;
            let current_head = transaction
                .run_with_options(&["rev-parse", "HEAD"], GitRunOptions::default_read())
                .await?;
            if !current_head.eq_ignore_ascii_case(expected_head) {
                return Err(format!(
                    "Cannot amend because HEAD changed since the commit was loaded (expected {}, found {}). Refresh and try again.",
                    short_oid(expected_head),
                    short_oid(&current_head)
                ));
            }
        }

        let message = Self::build_commit_message(commit_meta);

        let mut args = vec!["commit", "-F", "-"];
        if allow_empty {
            args.push("--allow-empty");
            // `--allow-empty` alone still commits staged changes. `--only` with no
            // paths makes Git create the empty commit from HEAD while preserving
            // the user's real index exactly as it was.
            args.push("--only");
        }
        if amend {
            args.push("--amend");
        }

        transaction
            .run_with_input(&args, &message, GitRunOptions::default_read())
            .await
            .map_err(|err| normalize_commit_error(&err))?;

        let commit_id = transaction
            .run_with_options(&["rev-parse", "HEAD"], GitRunOptions::default_read())
            .await?;

        self.ctx.cache.invalidate_all();
        drop(transaction);
        Ok(commit_id)
    }

    #[logger::logger]
    pub async fn commit_authors(&self) -> Result<Vec<Author>, String> {
        // Keep libgit2's non-Send config handle out of the future across the Git log await.
        let configured_author = {
            let repo = git2::Repository::open(&self.ctx.repo_path)
                .map_err(|err| format!("Failed to open repository: {err}"))?;
            let config = repo
                .config()
                .map_err(|err| format!("Failed to read Git configuration: {err}"))?;
            match (
                config.get_string("user.name"),
                config.get_string("user.email"),
            ) {
                (Ok(name), Ok(email)) => Some((name, email)),
                _ => None,
            }
        };

        let mut authors = Vec::new();
        let mut seen_emails = HashSet::new();

        if let Some((name, email)) = configured_author {
            push_unique_author(&mut authors, &mut seen_emails, name, email);
        }

        let format_arg = format!("--format={COMMIT_STANDARD_FORMAT}");
        let output = self
            .ctx
            .runner
            .run_with_options(
                &[
                    "log",
                    "--all",
                    "-n",
                    RECENT_COMMIT_AUTHOR_LIMIT,
                    &format_arg,
                ],
                GitRunOptions::default_read(),
            )
            .await?;

        for commit in parse_history_records(&output)? {
            push_unique_author(
                &mut authors,
                &mut seen_emails,
                commit.authors.author.name,
                commit.authors.author.email,
            );
            for co_author in commit.authors.co_authors {
                push_unique_author(
                    &mut authors,
                    &mut seen_emails,
                    co_author.name,
                    co_author.email,
                );
            }
        }

        Ok(authors)
    }

    fn build_commit_message(commit_meta: &CommitMessage) -> String {
        let mut msg = String::new();
        let description = commit_meta
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty());

        msg.push_str(commit_meta.title.trim());
        msg.push('\n');

        if let Some(description) = description {
            msg.push('\n');
            msg.push_str(description);
            msg.push('\n');
        }

        if !commit_meta.co_authors.is_empty() {
            // A trailer block is one paragraph. Keep the usual blank separator
            // after prose, but extend an existing block instead of splitting it.
            if description.is_none_or(|description| !ends_with_git_trailer(description)) {
                msg.push('\n');
            }
            for (name, email) in &commit_meta.co_authors {
                msg.push_str(&format!(
                    "Co-authored-by: {} <{}>\n",
                    name.trim(),
                    email.trim()
                ));
            }
        }

        msg
    }
}

fn ends_with_git_trailer(description: &str) -> bool {
    description
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .is_some_and(is_git_trailer_line)
}

fn is_git_trailer_line(line: &str) -> bool {
    let Some((token, value)) = line.split_once(':') else {
        return false;
    };
    let mut token_chars = token.chars();

    token_chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric())
        && token_chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        && !value.trim_start().is_empty()
}

fn validate_commit_request(
    commit_meta: &CommitMessage,
    allow_empty: bool,
    amend: bool,
    expected_head: Option<&str>,
) -> Result<(), String> {
    if amend && allow_empty {
        return Err("Amend and allow-empty cannot be used together".to_string());
    }
    match (amend, expected_head.map(str::trim)) {
        (true, None | Some("")) => {
            return Err("Expected HEAD is required when amending a commit".to_string());
        }
        (false, Some(_)) => {
            return Err("Expected HEAD can only be provided when amending a commit".to_string());
        }
        _ => {}
    }

    let title = commit_meta.title.trim();
    if title.is_empty() {
        return Err("Commit summary is required".to_string());
    }
    if title.contains('\n') || title.contains('\r') {
        return Err("Commit summary must be a single line".to_string());
    }

    for (index, (name, email)) in commit_meta.co_authors.iter().enumerate() {
        validate_co_author(name, email)
            .map_err(|err| format!("Invalid co-author {}: {err}", index + 1))?;
    }

    Ok(())
}

fn validate_co_author(name: &str, email: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("name is required".to_string());
    }
    if name.contains(['<', '>']) || name.chars().any(char::is_control) {
        return Err("name contains characters that are not valid in a Git trailer".to_string());
    }

    // Check the untrimmed input first so leading or trailing control characters
    // cannot be normalized into an apparently valid trailer address.
    if email.chars().any(char::is_control) {
        return Err("email contains characters that are not valid in a Git trailer".to_string());
    }

    let email = email.trim();
    if email.is_empty() {
        return Err("email is required".to_string());
    }
    if email.contains(['<', '>']) || email.chars().any(char::is_whitespace) {
        return Err("email contains characters that are not valid in a Git trailer".to_string());
    }

    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err("email must contain one @ with text on both sides".to_string());
    }

    Ok(())
}

fn validate_commit_operation(
    operation: &RepoOperation,
    allow_empty: bool,
    amend: bool,
) -> Result<(), String> {
    if !operation.conflict_paths.is_empty() {
        let action = if amend {
            "amend a commit"
        } else if allow_empty {
            "create an empty commit"
        } else {
            "create a commit"
        };
        return Err(format!(
            "Cannot {action} while {} has unresolved conflicts; resolve all conflicted paths first",
            operation_label(&operation.kind)
        ));
    }

    if allow_empty && operation.kind != RepoOperationKind::Clean {
        return Err(format!(
            "Cannot create an empty commit while {} is in progress",
            operation_label(&operation.kind)
        ));
    }

    if amend {
        let clean = operation.kind == RepoOperationKind::Clean;
        let rebase_edit = operation.is_rebasing
            && operation.pause_reason == Some(RebasePauseReason::Edit)
            && operation.conflict_paths.is_empty();
        if !clean && !rebase_edit {
            return Err(format!(
                "Cannot amend a commit while {} is in progress",
                operation_label(&operation.kind)
            ));
        }
        return Ok(());
    }

    if operation.is_rebasing
        || matches!(
            operation.kind,
            RepoOperationKind::ApplyMailbox | RepoOperationKind::Other
        )
    {
        return Err(format!(
            "Cannot create a commit while {} is in progress; use the operation controls instead",
            operation_label(&operation.kind)
        ));
    }

    Ok(())
}

fn operation_label(kind: &RepoOperationKind) -> &'static str {
    match kind {
        RepoOperationKind::Clean => "no operation",
        RepoOperationKind::Merge => "a merge",
        RepoOperationKind::Revert => "a revert",
        RepoOperationKind::CherryPick => "a cherry-pick",
        RepoOperationKind::Bisect => "a bisect",
        RepoOperationKind::Rebase
        | RepoOperationKind::RebaseInteractive
        | RepoOperationKind::RebaseMerge => "a rebase",
        RepoOperationKind::ApplyMailbox => "mailbox application",
        RepoOperationKind::Other => "another Git operation",
    }
}

fn push_unique_author(
    authors: &mut Vec<Author>,
    seen_emails: &mut HashSet<String>,
    name: String,
    email: String,
) {
    let name = name.trim();
    let email = email.trim();
    if validate_co_author(name, email).is_err() {
        return;
    }

    if seen_emails.insert(email.to_ascii_lowercase()) {
        authors.push(Author {
            name: name.to_string(),
            email: email.to_string(),
        });
    }
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
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

    fn operation(kind: RepoOperationKind) -> RepoOperation {
        let mut operation = RepoOperation::clean();
        operation.kind = kind;
        operation
    }

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

    #[test]
    fn commit_message_separates_prose_from_co_author_trailers() {
        let message = CommitService::build_commit_message(&CommitMessage {
            title: "Pair change".to_string(),
            description: Some("Explains the change".to_string()),
            co_authors: vec![("Alice".to_string(), "alice@example.com".to_string())],
        });

        assert_eq!(
            message,
            "Pair change\n\nExplains the change\n\nCo-authored-by: Alice <alice@example.com>\n"
        );
    }

    #[test]
    fn commit_message_extends_existing_trailer_block() {
        let message = CommitService::build_commit_message(&CommitMessage {
            title: "Pair change".to_string(),
            description: Some(
                "Explains the change\n\nSigned-off-by: Ruru <ruru@example.com>".to_string(),
            ),
            co_authors: vec![("Alice".to_string(), "alice@example.com".to_string())],
        });

        assert_eq!(
            message,
            "Pair change\n\nExplains the change\n\nSigned-off-by: Ruru <ruru@example.com>\nCo-authored-by: Alice <alice@example.com>\n"
        );
    }

    #[test]
    fn validate_co_author_rejects_invalid_trailer_characters() {
        assert!(validate_co_author("", "alice@example.com").is_err());
        assert!(validate_co_author("Alice <Admin>", "alice@example.com").is_err());
        assert!(validate_co_author("Alice\nCo-authored-by: Mallory", "alice@example.com").is_err());
        assert!(validate_co_author("Alice", "alice @example.com").is_err());
        assert!(validate_co_author("Alice", "alice@example.com\nInjected").is_err());
        assert!(validate_co_author("Alice", "alice@example.com\n").is_err());
        assert!(validate_co_author("Alice", "alice@example.com\u{0007}").is_err());
        assert!(validate_co_author("Alice", "alice@example.com").is_ok());
    }

    #[test]
    fn request_validation_requires_expected_head_only_for_amend() {
        let message = CommitMessage {
            title: "Valid summary".to_string(),
            description: None,
            co_authors: vec![],
        };

        assert!(validate_commit_request(&message, false, true, None).is_err());
        assert!(validate_commit_request(&message, false, true, Some("  ")).is_err());
        assert!(validate_commit_request(&message, false, false, Some("abc123")).is_err());
        assert!(validate_commit_request(&message, false, true, Some("abc123")).is_ok());
    }

    #[test]
    fn operation_validation_preserves_standard_completion_commits() {
        for kind in [
            RepoOperationKind::Clean,
            RepoOperationKind::Merge,
            RepoOperationKind::Revert,
            RepoOperationKind::CherryPick,
            RepoOperationKind::Bisect,
        ] {
            assert!(validate_commit_operation(&operation(kind), false, false).is_ok());
        }
    }

    #[test]
    fn operation_validation_blocks_ordinary_commits_in_incompatible_states() {
        for kind in [
            RepoOperationKind::Rebase,
            RepoOperationKind::RebaseInteractive,
            RepoOperationKind::RebaseMerge,
            RepoOperationKind::ApplyMailbox,
            RepoOperationKind::Other,
        ] {
            let mut operation = operation(kind);
            operation.is_rebasing = matches!(
                operation.kind,
                RepoOperationKind::Rebase
                    | RepoOperationKind::RebaseInteractive
                    | RepoOperationKind::RebaseMerge
            );
            assert!(validate_commit_operation(&operation, false, false).is_err());
        }
    }

    #[test]
    fn operation_validation_blocks_completion_commits_until_conflicts_are_resolved() {
        for kind in [
            RepoOperationKind::Merge,
            RepoOperationKind::Revert,
            RepoOperationKind::CherryPick,
        ] {
            let mut operation = operation(kind);
            operation.conflict_paths.push("conflicted.txt".to_string());

            let error = validate_commit_operation(&operation, false, false).unwrap_err();
            assert!(error.contains("unresolved conflicts"));

            operation.conflict_paths.clear();
            assert!(validate_commit_operation(&operation, false, false).is_ok());
        }
    }

    #[test]
    fn operation_validation_only_allows_special_modes_when_compatible() {
        assert!(
            validate_commit_operation(&operation(RepoOperationKind::Clean), true, false).is_ok()
        );
        assert!(
            validate_commit_operation(&operation(RepoOperationKind::Merge), true, false).is_err()
        );
        assert!(
            validate_commit_operation(&operation(RepoOperationKind::Merge), false, true).is_err()
        );

        let mut clean_with_conflicts = operation(RepoOperationKind::Clean);
        clean_with_conflicts
            .conflict_paths
            .push("stash-conflict.txt".to_string());
        assert!(validate_commit_operation(&clean_with_conflicts, false, false).is_err());
        assert!(validate_commit_operation(&clean_with_conflicts, true, false).is_err());
        assert!(validate_commit_operation(&clean_with_conflicts, false, true).is_err());

        let mut rebase_edit = operation(RepoOperationKind::RebaseInteractive);
        rebase_edit.is_rebasing = true;
        rebase_edit.pause_reason = Some(RebasePauseReason::Edit);
        assert!(validate_commit_operation(&rebase_edit, false, true).is_ok());
        assert!(validate_commit_operation(&rebase_edit, false, false).is_err());

        rebase_edit
            .conflict_paths
            .push("conflicted.txt".to_string());
        assert!(validate_commit_operation(&rebase_edit, false, true).is_err());
    }
}
