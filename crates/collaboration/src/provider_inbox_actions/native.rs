use super::*;
use crate::{commands::*, effective::ItemIntentPatch, *};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const KIND: &str = "provider.inbox_action";
pub(crate) fn invalid() -> CollaborationError {
    CollaborationError::invalid("Invalid provider inbox action")
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Payload {
    pub instance: String,
    pub actor: String,
    pub native_id: String,
    pub project: String,
    pub updated_at: String,
    pub activity: String,
    pub view: String,
    pub action: ProviderInboxAction,
    pub subject_type: String,
    pub native_action: String,
}
impl Payload {
    pub fn parse(bytes: &[u8]) -> Result<Self, CollaborationError> {
        if bytes.len() > 8192 {
            return Err(invalid());
        }
        let mut rest = bytes;
        let mut fields = Vec::new();
        for tag in 1u16..=10 {
            if rest.len() < 6 || u16::from_be_bytes(rest[..2].try_into().unwrap()) != tag {
                return Err(invalid());
            }
            let len = u32::from_be_bytes(rest[2..6].try_into().unwrap()) as usize;
            rest = &rest[6..];
            if len > 2048 || len > rest.len() {
                return Err(invalid());
            }
            fields.push(
                std::str::from_utf8(&rest[..len])
                    .map_err(|_| invalid())?
                    .to_string(),
            );
            rest = &rest[len..];
        }
        if !rest.is_empty() {
            return Err(invalid());
        }
        let p = Self {
            instance: fields[0].clone(),
            actor: fields[1].clone(),
            native_id: fields[2].clone(),
            project: fields[3].clone(),
            updated_at: fields[4].clone(),
            activity: fields[5].clone(),
            view: fields[6].clone(),
            action: match fields[7].as_str() {
                "mark_read" => ProviderInboxAction::MarkRead,
                "mark_done" => ProviderInboxAction::MarkDone,
                _ => return Err(invalid()),
            },
            subject_type: fields[8].clone(),
            native_action: fields[9].clone(),
        };
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<(), CollaborationError> {
        let digit = |s: &str| s.parse::<u64>().is_ok_and(|n| n > 0 && n.to_string() == s);
        if !digit(&self.native_id)
            || !digit(&self.project)
            || self.actor.is_empty()
            || self.actor.len() > 1024
            || self.activity.len() != 64
            || !self.activity.bytes().all(|b| b.is_ascii_hexdigit())
            || self.view.is_empty()
            || self.view.len() > 128
            || chrono::DateTime::parse_from_rfc3339(&self.updated_at).is_err()
            || self.subject_type.len() > 128
            || self.native_action.len() > 256
        {
            return Err(invalid());
        }
        if !matches!(
            (self.instance.as_str(), self.action),
            ("github:https://github.com/", ProviderInboxAction::MarkRead)
                | ("gitlab:https://gitlab.com/", ProviderInboxAction::MarkDone)
        ) {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn patch(&self) -> ItemIntentPatch {
        match self.action {
            ProviderInboxAction::MarkRead => ItemIntentPatch {
                unread: Some(false),
                ..Default::default()
            },
            ProviderInboxAction::MarkDone => ItemIntentPatch {
                state: Some("done".into()),
                ..Default::default()
            },
        }
    }
    pub fn matches_request(&self, r: &QueueProviderInboxActionRequest) -> bool {
        self.view == r.authorization_view
            && self.activity == r.expected_activity_version
            && self.action == r.action
    }
}
impl CommandPayloadCodec for Payload {
    const OPERATION_KIND: &'static str = KIND;
    const PAYLOAD_VERSION: u32 = 1;
    fn encode_payload(&self, f: &mut CanonicalFields) -> Result<(), CollaborationError> {
        self.validate()?;
        let values: [&str; 10] = [
            &self.instance,
            &self.actor,
            &self.native_id,
            &self.project,
            &self.updated_at,
            &self.activity,
            &self.view,
            match self.action {
                ProviderInboxAction::MarkRead => "mark_read",
                ProviderInboxAction::MarkDone => "mark_done",
            },
            &self.subject_type,
            &self.native_action,
        ];
        for (tag, value) in values.into_iter().enumerate() {
            f.string(tag as u16 + 1, value)?;
        }
        Ok(())
    }
}
pub(crate) fn activity(item: &RemoteItem) -> Result<String, CollaborationError> {
    let mut h = Sha256::new();
    h.update(b"gitru.provider-inbox-activity.v1\0");
    h.update(serde_json::to_vec(item).map_err(|_| invalid())?);
    Ok(format!("{:x}", h.finalize()))
}
pub(crate) fn seal(
    request: &QueueProviderInboxActionRequest,
    payload: Payload,
    repository: Option<String>,
) -> Result<CommandSubmission, CollaborationError> {
    seal_command(CommandDraft {
        command_id: request.command_id.clone(),
        account_id: request.account_id.clone(),
        authorization_epoch: request.authorization_epoch.clone(),
        target: CommandTarget::new(
            CommandTargetKind::Notification,
            &request.subject_id,
            repository,
        )?,
        payload,
        guards: vec![],
        dependencies: vec![],
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frame {
    pub item: RemoteItem,
    pub view: String,
    pub instance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Observation {
    pub updated_at: String,
    pub applied: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proof {
    pub payload: Payload,
    pub frame: Frame,
    pub observation: Observation,
}
