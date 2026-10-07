//! An installation is a trust boundary, not a provider-family fallback.
use std::{collections::HashMap, sync::Arc};

use crate::{CollaborationError, ErrorCode, domain::*};

use super::{CollaborationProvider, FeedKind};

impl ProviderInstance {
    pub fn new(provider: ProviderKind, base_url: &str) -> Result<Self, CollaborationError> {
        // Reject spellings which URL parsing would silently normalize across an
        // installation boundary. Production instances are explicitly HTTPS.
        if base_url.len() > 2048
            || base_url.trim() != base_url
            || base_url.contains('\\')
            || base_url.contains('%')
            || base_url.chars().any(char::is_control)
            || base_url.split('/').any(|p| p == "." || p == "..")
        {
            return Err(CollaborationError::invalid(
                "Invalid provider installation URL",
            ));
        }
        let mut url = url::Url::parse(base_url)
            .map_err(|_| CollaborationError::invalid("Invalid provider installation URL"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path().contains('%')
            || url.path().contains("//")
            || url.path().contains('\\')
        {
            return Err(CollaborationError::invalid(
                "Invalid provider installation URL",
            ));
        }
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
        let base_url = url.to_string();
        let provider_name = match provider {
            ProviderKind::Github => "github",
            ProviderKind::Gitlab => "gitlab",
            ProviderKind::BitbucketCloud => "bitbucket_cloud",
            ProviderKind::BitbucketDc => "bitbucket_dc",
        };
        Ok(Self {
            id: format!("{provider_name}:{base_url}"),
            provider,
            base_url,
        })
    }

    pub fn for_account(account: &RemoteAccount) -> Result<Self, CollaborationError> {
        let base = if account.host.contains("://") {
            account.host.clone()
        } else {
            format!("https://{}/", account.host.trim_end_matches('/'))
        };
        Self::new(account.provider, &base)
    }

    pub fn public(provider: ProviderKind) -> Self {
        let base = match provider {
            ProviderKind::Github => "https://github.com/",
            ProviderKind::Gitlab => "https://gitlab.com/",
            ProviderKind::BitbucketCloud => "https://bitbucket.org/",
            ProviderKind::BitbucketDc => "https://bitbucket.invalid/",
        };
        Self::new(provider, base).expect("constant provider installation")
    }
}

#[derive(Debug, Clone)]
pub struct ProviderProfile {
    pub facets: Vec<FacetCapability>,
    pub inbox_semantics: InboxSemantics,
}

pub const FACETS: [ResourceFacet; 14] = [
    ResourceFacet::Repositories,
    ResourceFacet::PullRequests,
    ResourceFacet::Issues,
    ResourceFacet::Inbox,
    ResourceFacet::PullDetails,
    ResourceFacet::IssueDetails,
    ResourceFacet::Comments,
    ResourceFacet::Reviews,
    ResourceFacet::Checks,
    ResourceFacet::Participants,
    ResourceFacet::Tasks,
    ResourceFacet::PullCommits,
    ResourceFacet::PullFiles,
    ResourceFacet::Merge,
];

impl ProviderProfile {
    pub fn read_only(inbox_semantics: InboxSemantics, inbox_supported: bool) -> Self {
        let facets = FACETS
            .into_iter()
            .map(|facet| {
                let (state, reason) = match facet {
                    ResourceFacet::Repositories
                    | ResourceFacet::PullRequests
                    | ResourceFacet::Issues => (CapabilityState::Supported, None),
                    ResourceFacet::Inbox if inbox_semantics == InboxSemantics::None => (
                        CapabilityState::Unsupported,
                        Some(CapabilityReason::ProviderSemantics),
                    ),
                    ResourceFacet::Inbox if !inbox_supported => (
                        CapabilityState::Unavailable,
                        Some(CapabilityReason::MissingScope),
                    ),
                    ResourceFacet::Inbox => (CapabilityState::Supported, None),
                    _ => (
                        CapabilityState::Unsupported,
                        Some(CapabilityReason::NotImplemented),
                    ),
                };
                FacetCapability {
                    facet,
                    state,
                    reason,
                }
            })
            .collect();
        Self {
            facets,
            inbox_semantics,
        }
    }

    pub fn facet(&self, facet: ResourceFacet) -> FacetCapability {
        self.facets
            .iter()
            .find(|c| c.facet == facet)
            .cloned()
            .unwrap_or(FacetCapability {
                facet,
                state: CapabilityState::Unsupported,
                reason: Some(CapabilityReason::NotImplemented),
            })
    }

    pub fn unavailable(reason: CapabilityReason) -> Self {
        Self {
            facets: FACETS
                .into_iter()
                .map(|facet| FacetCapability {
                    facet,
                    state: CapabilityState::Unavailable,
                    reason: Some(reason),
                })
                .collect(),
            inbox_semantics: InboxSemantics::None,
        }
    }
}

impl FeedKind {
    pub fn facet(self) -> ResourceFacet {
        match self {
            Self::Repositories => ResourceFacet::Repositories,
            Self::PullRequests => ResourceFacet::PullRequests,
            Self::Issues => ResourceFacet::Issues,
            Self::Notifications => ResourceFacet::Inbox,
        }
    }
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    adapters: HashMap<String, (ProviderInstance, Arc<dyn CollaborationProvider>)>,
    delivery: HashMap<(String, String, u32), Arc<dyn crate::delivery::CommandDeliveryPolicy>>,
    recovery: HashMap<
        (String, String, u32),
        Arc<dyn crate::command_recovery::policy::CommandRecoveryPolicy>,
    >,
}

impl ProviderRegistry {
    #[allow(dead_code, reason = "operation codecs land in downstream issues")]
    pub(crate) fn register_recovery(
        &mut self,
        instance: &ProviderInstance,
        policy: Arc<dyn crate::command_recovery::policy::CommandRecoveryPolicy>,
    ) -> Result<(), CollaborationError> {
        self.adapter(instance)?;
        let key = (
            instance.id.clone(),
            policy.operation_kind().to_owned(),
            policy.payload_version(),
        );
        if policy.instance_id() != instance.id
            || !self.delivery.contains_key(&key)
            || self.recovery.contains_key(&key)
        {
            return Err(CollaborationError::invalid(
                "Missing delivery or duplicate recovery policy",
            ));
        }
        self.recovery.insert(key, policy);
        Ok(())
    }
    pub(crate) fn recovery_policy(
        &self,
        instance: &ProviderInstance,
        kind: &str,
        version: u32,
    ) -> Option<Arc<dyn crate::command_recovery::policy::CommandRecoveryPolicy>> {
        self.adapter(instance).ok()?;
        self.recovery
            .get(&(instance.id.clone(), kind.into(), version))
            .cloned()
    }
    #[allow(dead_code, reason = "operation codecs land in downstream issues")]
    pub(crate) fn register_delivery(
        &mut self,
        instance: &ProviderInstance,
        policy: Arc<dyn crate::delivery::CommandDeliveryPolicy>,
    ) -> Result<(), CollaborationError> {
        self.adapter(instance)?;
        let kind = policy.operation_kind();
        let version = policy.payload_version();
        if kind.is_empty()
            || kind.len() > 128
            || version == 0
            || !kind.as_bytes()[0].is_ascii_lowercase()
            || !kind.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            })
        {
            return Err(CollaborationError::invalid("Invalid delivery policy"));
        }
        let key = (instance.id.clone(), kind.into(), version);
        if self.delivery.contains_key(&key) {
            return Err(CollaborationError::invalid("Duplicate delivery policy"));
        }
        self.delivery.insert(key, policy);
        Ok(())
    }
    pub(crate) fn delivery_policy(
        &self,
        instance: &ProviderInstance,
        kind: &str,
        version: u32,
    ) -> Option<Arc<dyn crate::delivery::CommandDeliveryPolicy>> {
        self.adapter(instance).ok()?;
        self.delivery
            .get(&(instance.id.clone(), kind.into(), version))
            .cloned()
    }
    pub fn register(
        &mut self,
        adapter: Arc<dyn CollaborationProvider>,
    ) -> Result<(), CollaborationError> {
        let instance = adapter.instance();
        if ProviderInstance::new(instance.provider, &instance.base_url)? != instance
            || instance.provider != adapter.kind()
            || self.adapters.contains_key(&instance.id)
        {
            return Err(CollaborationError::invalid(
                "Invalid or duplicate provider adapter",
            ));
        }
        self.adapters
            .insert(instance.id.clone(), (instance, adapter));
        Ok(())
    }

    pub fn adapter(
        &self,
        instance: &ProviderInstance,
    ) -> Result<Arc<dyn CollaborationProvider>, CollaborationError> {
        self.adapters
            .get(&instance.id)
            .filter(|(registered, _)| registered == instance)
            .map(|(_, adapter)| adapter.clone())
            .ok_or_else(|| {
                CollaborationError::new(
                    ErrorCode::Unsupported,
                    "No adapter is installed for this provider instance",
                )
            })
    }
}
