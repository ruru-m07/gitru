use super::*;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

struct Grant {
    context: PullCreationContext,
    frame: Frame,
    actor: String,
    owner: PullCreationOwner,
    expires: Instant,
    request: Option<SubmitPullRequest>,
}
pub(super) struct Grants {
    entries: Mutex<HashMap<String, Grant>>,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
}
impl Default for Grants {
    fn default() -> Self {
        Self::new(Arc::new(Instant::now))
    }
}
impl Grants {
    pub(super) fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
    pub(super) fn new(now: Arc<dyn Fn() -> Instant + Send + Sync>) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            now,
        }
    }
    pub(super) fn issue(
        &self,
        a: &RemoteAccount,
        f: &Frame,
        key: &PullDraftKey,
        source_oid: String,
        base_oid: String,
        owner: &PullCreationOwner,
    ) -> Result<PullCreationContext, CollaborationError> {
        owner.validate()?;
        let now = (self.now)();
        let mut entries = self.entries.lock().map_err(|_| invalid())?;
        entries.retain(|_, g| g.expires > now);
        if entries.len() >= MAX_GRANTS {
            return Err(CollaborationError::new(
                ErrorCode::Busy,
                "Too many active pull request previews",
            ));
        }
        let context = PullCreationContext {
            key: key.clone(),
            draft_generation: f.draft_generation.clone(),
            authorization_epoch: a.authorization_epoch.clone(),
            authorization_view: f.authorization_view.clone(),
            source_oid,
            base_oid,
            grant_id: uuid::Uuid::new_v4().to_string(),
        };
        entries.insert(
            context.grant_id.clone(),
            Grant {
                context: context.clone(),
                frame: f.clone(),
                actor: a.actor_id.clone(),
                owner: owner.clone(),
                expires: now + Duration::from_secs(GRANT_SECONDS),
                request: None,
            },
        );
        Ok(context)
    }
    pub(super) fn arm(
        &self,
        a: &RemoteAccount,
        r: &SubmitPullRequest,
        owner: &PullCreationOwner,
    ) -> Result<Option<Frame>, CollaborationError> {
        let now = (self.now)();
        let mut entries = self.entries.lock().map_err(|_| invalid())?;
        entries.retain(|_, g| g.expires > now);
        let Some(g) = entries.get_mut(&r.context.grant_id) else {
            return Ok(None);
        };
        owner.validate()?;
        if !g.owner.same(owner)
            || g.owner.validate().is_err()
            || g.context != r.context
            || g.actor != a.actor_id
            || g.request.as_ref().is_some_and(|old| old != r)
        {
            return Err(invalid());
        }
        g.request = Some(r.clone());
        Ok(Some(g.frame.clone()))
    }
    pub(super) fn live(&self, a: &RemoteAccount, r: &SubmitPullRequest) -> bool {
        self.entries.lock().ok().is_some_and(|entries| {
            entries.get(&r.context.grant_id).is_some_and(|g| {
                g.expires > (self.now)()
                    && g.owner.validate().is_ok()
                    && g.context == r.context
                    && g.actor == a.actor_id
                    && a.id == r.context.key.account_id
                    && a.authorization_epoch == r.context.authorization_epoch
                    && g.request.as_ref() == Some(r)
            })
        })
    }
    pub(super) fn consume(&self, a: &RemoteAccount, r: &SubmitPullRequest) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let live = entries.get(&r.context.grant_id).is_some_and(|g| {
            g.expires > (self.now)()
                && g.owner.validate().is_ok()
                && g.context == r.context
                && g.actor == a.actor_id
                && a.id == r.context.key.account_id
                && a.authorization_epoch == r.context.authorization_epoch
                && g.request.as_ref() == Some(r)
        });
        if live {
            entries.remove(&r.context.grant_id);
        }
        live
    }
}
