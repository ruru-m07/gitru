//! Native-only selected-file authority. A renderer can supply only the lookup
//! key; storage resolves these records before any credential is consulted.
use super::*;

#[derive(Debug, Clone)]
pub struct PullFileSourceRequest {
    pub account: RemoteAccount,
    pub repository: RemoteRepository,
    pub subject: RemoteItem,
    pub binding: PullFileBinding,
    pub source: PullFileSource,
}
impl From<&PullFileCollectionRequest> for PullFileSourceRequest {
    fn from(request: &PullFileCollectionRequest) -> Self {
        Self {
            account: request.account.clone(),
            repository: request.repository.clone(),
            subject: request.subject.clone(),
            binding: request.binding.clone(),
            source: request.source.clone(),
        }
    }
}
impl PullFileSourceRequest {
    pub fn validate(&self) -> Result<(), CollaborationError> {
        self.binding.validate()?;
        self.source.validate()?;
        if self.account.state != AccountState::Active
            || self.account.id.is_empty()
            || self
                .account
                .authorization_epoch
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0)
                .map(|n| n.to_string())
                .as_deref()
                != Some(&self.account.authorization_epoch)
            || self.repository.account_id != self.account.id
            || self.subject.account_id != self.account.id
            || self.subject.repository_id.as_deref() != Some(self.repository.id.as_str())
            || self.subject.kind != RemoteItemKind::PullRequest
            || self.binding.repository_id != self.repository.id
            || self.binding.repository_provider_id != self.repository.provider_id
            || self.binding.context.base_repository_provider_id != self.repository.provider_id
            || self.binding.pull_id != self.subject.id
            || self.binding.pull_provider_id != self.subject.provider_id
            || self.binding.number != self.subject.number
            || self.source.strategy.provider() != Some(self.account.provider)
        {
            return Err(CollaborationError::invalid(
                "Invalid native selected file authority",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct PullFileSelectedRequest {
    pub resource: PullFileSourceRequest,
    pub membership: PullFileMembershipReceipt,
    pub file: PullFile,
}
impl PullFileSelectedRequest {
    pub fn validate(&self) -> Result<(), CollaborationError> {
        self.resource.validate()?;
        self.membership.validate()?;
        self.file.validate()?;
        if self.membership.account_id != self.resource.account.id
            || self.membership.authorization_epoch != self.resource.account.authorization_epoch
            || self.membership.subject_id != self.resource.subject.id
            || self.membership.context != self.resource.binding.context
            || self.file.context != self.membership.context
            || self.file.file_key != self.membership.file_key
            || self.file.file.identity != self.membership.identity
        {
            return Err(CollaborationError::invalid(
                "Invalid selected file membership",
            ));
        }
        Ok(())
    }
}
/// Bounded content without admission authority. Runtime adds fresh provider
/// range evidence, and the store rechecks the exact active membership.
#[derive(Debug, Clone)]
pub struct PullFileArtifactRead {
    pub content_state: PullFileContentState,
    pub unified_text: Option<String>,
    pub binary_hint: PullFileFlag,
    pub cooldown_seconds: Option<u64>,
}
