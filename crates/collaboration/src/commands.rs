//! Immutable, operation-typed command submissions.
//!
//! Provider operations implement the crate-private codecs below. The generic
//! envelope never accepts renderer JSON, provider routes, or caller-computed
//! hashes.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "operation codecs land with their owning command issues"
    )
)]

use crate::CollaborationError;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

pub const COMMAND_ENVELOPE_VERSION: u32 = 1;
pub const MAX_COMMAND_ENVELOPE_BYTES: usize = 256 * 1024;
pub const MAX_COMMAND_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_COMMAND_OPERATION_KIND_BYTES: usize = 128;
pub const MAX_COMMAND_GUARDS: usize = 32;
pub const MAX_COMMAND_DEPENDENCIES: usize = 64;

const MAX_COMMAND_IDENTIFIER_BYTES: usize = 1024;
const HASH_DOMAIN: &[u8] = b"gitru.collaboration.command-submission.sha256.v1";

type Result<T> = std::result::Result<T, CollaborationError>;

fn invalid_command() -> CollaborationError {
    CollaborationError::invalid("Invalid or unbounded collaboration command")
}

fn validate_identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_COMMAND_IDENTIFIER_BYTES
        || value.as_bytes().contains(&0)
    {
        Err(invalid_command())
    } else {
        Ok(())
    }
}

fn validate_kind(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_COMMAND_OPERATION_KIND_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
        || !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
    {
        Err(invalid_command())
    } else {
        Ok(())
    }
}

fn validate_uuid(value: &str) -> Result<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| invalid_command())?;
    if parsed.hyphenated().to_string() != value {
        return Err(invalid_command());
    }
    Ok(())
}

fn authorization_epoch(value: &str) -> Result<u64> {
    if value.is_empty() || value.len() > 19 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_command());
    }
    let epoch = value.parse::<u64>().map_err(|_| invalid_command())?;
    if epoch == 0 || epoch > i64::MAX as u64 || epoch.to_string() != value {
        return Err(invalid_command());
    }
    Ok(epoch)
}

/// Stable command target tags are part of the persisted envelope contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandTargetKind {
    Repository,
    PullRequest,
    Issue,
    Notification,
}

impl CommandTargetKind {
    pub(crate) const fn storage_name(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::PullRequest => "pull_request",
            Self::Issue => "issue",
            Self::Notification => "notification",
        }
    }

    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Repository => 1,
            Self::PullRequest => 2,
            Self::Issue => 3,
            Self::Notification => 4,
        }
    }
}

/// Canonical local identity selected by an operation-specific admission path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTarget {
    kind: CommandTargetKind,
    id: String,
    repository_id: Option<String>,
}

impl CommandTarget {
    pub(crate) fn new(
        kind: CommandTargetKind,
        id: impl Into<String>,
        repository_id: Option<String>,
    ) -> Result<Self> {
        let id = id.into();
        validate_identifier(&id)?;
        if let Some(repository_id) = &repository_id {
            validate_identifier(repository_id)?;
        }
        Ok(Self {
            kind,
            id,
            repository_id,
        })
    }

    pub fn kind(&self) -> CommandTargetKind {
        self.kind
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn repository_id(&self) -> Option<&str> {
        self.repository_id.as_deref()
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let mut fields = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
        fields.bytes(1, &[self.kind.canonical_tag()])?;
        fields.string(2, &self.id)?;
        fields.optional_string(3, self.repository_id.as_deref())?;
        Ok(fields.finish())
    }
}

/// Native operation identity and its immutable payload schema version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOperation {
    kind: String,
    payload_version: u32,
}

impl CommandOperation {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn payload_version(&self) -> u32 {
        self.payload_version
    }
}

/// Type-erased only after an operation-specific guard codec has validated and
/// encoded the observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandGuard {
    kind: String,
    version: u32,
    canonical_bytes: Vec<u8>,
}

impl CommandGuard {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let mut fields = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
        fields.string(1, &self.kind)?;
        fields.u32(2, self.version)?;
        fields.bytes(3, &self.canonical_bytes)?;
        Ok(fields.finish())
    }
}

/// Sealed immutable value accepted by the future command admission store.
/// Construction remains crate-private until a real operation exposes its own
/// typed API.
#[derive(Debug, Clone)]
pub struct CommandSubmission {
    command_id: String,
    account_id: String,
    authorization_epoch: String,
    operation: CommandOperation,
    target: CommandTarget,
    payload_bytes: Vec<u8>,
    guards: Vec<CommandGuard>,
    dependencies: Vec<String>,
    canonical_envelope: Vec<u8>,
    submission_hash: [u8; 32],
}

impl CommandSubmission {
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn authorization_epoch(&self) -> &str {
        &self.authorization_epoch
    }

    pub fn operation(&self) -> &CommandOperation {
        &self.operation
    }

    pub fn target(&self) -> &CommandTarget {
        &self.target
    }

    pub fn payload_bytes(&self) -> &[u8] {
        &self.payload_bytes
    }

    pub fn guards(&self) -> &[CommandGuard] {
        &self.guards
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub fn canonical_envelope(&self) -> &[u8] {
        &self.canonical_envelope
    }

    pub fn submission_hash(&self) -> &[u8; 32] {
        &self.submission_hash
    }

    pub fn submission_hash_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(64);
        for byte in self.submission_hash {
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        encoded
    }

    /// Idempotency compares the persisted bytes and every decomposed fact. The
    /// digest is corroborating evidence, never the equality authority.
    pub fn is_exact_retry_of(&self, other: &Self) -> bool {
        self.command_id == other.command_id
            && self.account_id == other.account_id
            && self.authorization_epoch == other.authorization_epoch
            && self.operation == other.operation
            && self.target == other.target
            && self.payload_bytes == other.payload_bytes
            && self.guards == other.guards
            && self.dependencies == other.dependencies
            && self.canonical_envelope == other.canonical_envelope
            && self.submission_hash == other.submission_hash
    }
}

#[derive(Clone)]
pub(crate) struct CommandDraft<P> {
    pub command_id: String,
    pub account_id: String,
    pub authorization_epoch: String,
    pub target: CommandTarget,
    pub payload: P,
    pub guards: Vec<CommandGuard>,
    pub dependencies: Vec<String>,
}

/// Implemented only by reviewed native operation modules. Each codec writes
/// fields in a fixed order; it cannot delegate canonicalization to JSON.
pub(crate) trait CommandPayloadCodec {
    const OPERATION_KIND: &'static str;
    const PAYLOAD_VERSION: u32;

    fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()>;
}

/// Implemented only by reviewed native guard types.
pub(crate) trait CommandGuardCodec {
    const GUARD_KIND: &'static str;
    const GUARD_VERSION: u32;

    fn encode_guard(&self, fields: &mut CanonicalFields) -> Result<()>;
}

pub(crate) fn seal_guard<G: CommandGuardCodec>(guard: &G) -> Result<CommandGuard> {
    validate_kind(G::GUARD_KIND)?;
    if G::GUARD_VERSION == 0 {
        return Err(invalid_command());
    }
    let mut fields = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
    guard.encode_guard(&mut fields)?;
    Ok(CommandGuard {
        kind: G::GUARD_KIND.into(),
        version: G::GUARD_VERSION,
        canonical_bytes: fields.finish(),
    })
}

pub(crate) fn seal_command<P: CommandPayloadCodec>(
    draft: CommandDraft<P>,
) -> Result<CommandSubmission> {
    validate_uuid(&draft.command_id)?;
    validate_identifier(&draft.account_id)?;
    let epoch = authorization_epoch(&draft.authorization_epoch)?;
    validate_kind(P::OPERATION_KIND)?;
    if P::PAYLOAD_VERSION == 0
        || draft.guards.len() > MAX_COMMAND_GUARDS
        || draft.dependencies.len() > MAX_COMMAND_DEPENDENCIES
    {
        return Err(invalid_command());
    }

    let mut unique_dependencies = HashSet::with_capacity(draft.dependencies.len());
    for dependency in &draft.dependencies {
        validate_uuid(dependency)?;
        if dependency == &draft.command_id || !unique_dependencies.insert(dependency.as_str()) {
            return Err(invalid_command());
        }
    }

    let mut payload = CanonicalFields::new(MAX_COMMAND_PAYLOAD_BYTES);
    draft.payload.encode_payload(&mut payload)?;
    let payload_bytes = payload.finish();
    if payload_bytes.len() > MAX_COMMAND_PAYLOAD_BYTES {
        return Err(invalid_command());
    }

    let operation = CommandOperation {
        kind: P::OPERATION_KIND.into(),
        payload_version: P::PAYLOAD_VERSION,
    };
    let target_bytes = draft.target.encode()?;
    let guard_bytes = encode_guards(&draft.guards)?;
    let dependency_bytes = encode_dependencies(&draft.dependencies)?;
    let mut envelope = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
    envelope.u32(1, COMMAND_ENVELOPE_VERSION)?;
    envelope.string(2, &draft.account_id)?;
    envelope.u64(3, epoch)?;
    envelope.string(4, &operation.kind)?;
    envelope.u32(5, operation.payload_version)?;
    envelope.bytes(6, &target_bytes)?;
    envelope.bytes(7, &payload_bytes)?;
    envelope.bytes(8, &guard_bytes)?;
    envelope.bytes(9, &dependency_bytes)?;
    let canonical_envelope = envelope.finish();
    if canonical_envelope.len() > MAX_COMMAND_ENVELOPE_BYTES {
        return Err(invalid_command());
    }

    let submission_hash = submission_hash(&canonical_envelope);
    Ok(CommandSubmission {
        command_id: draft.command_id,
        account_id: draft.account_id,
        authorization_epoch: draft.authorization_epoch,
        operation,
        target: draft.target,
        payload_bytes,
        guards: draft.guards,
        dependencies: draft.dependencies,
        canonical_envelope,
        submission_hash,
    })
}

pub(crate) fn encode_guards(guards: &[CommandGuard]) -> Result<Vec<u8>> {
    let mut sequence = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
    for (index, guard) in guards.iter().enumerate() {
        let tag = u16::try_from(index + 1).map_err(|_| invalid_command())?;
        sequence.bytes(tag, &guard.encode()?)?;
    }
    Ok(sequence.finish())
}

fn encode_dependencies(dependencies: &[String]) -> Result<Vec<u8>> {
    let mut sequence = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
    for (index, dependency) in dependencies.iter().enumerate() {
        let tag = u16::try_from(index + 1).map_err(|_| invalid_command())?;
        sequence.string(tag, dependency)?;
    }
    Ok(sequence.finish())
}

fn submission_hash(envelope: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((HASH_DOMAIN.len() as u32).to_be_bytes());
    digest.update(HASH_DOMAIN);
    digest.update((envelope.len() as u64).to_be_bytes());
    digest.update(envelope);
    digest.finalize().into()
}

/// Minimal deterministic field writer shared by operation and guard codecs.
/// Tags must be nonzero, unique and strictly increasing.
pub(crate) struct CanonicalFields {
    bytes: Vec<u8>,
    last_tag: u16,
    max_bytes: usize,
}

impl CanonicalFields {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            last_tag: 0,
            max_bytes,
        }
    }

    pub(crate) fn bytes(&mut self, tag: u16, value: &[u8]) -> Result<()> {
        if tag == 0 || tag <= self.last_tag {
            return Err(invalid_command());
        }
        let length = u32::try_from(value.len()).map_err(|_| invalid_command())?;
        let framed_length = 6usize
            .checked_add(value.len())
            .and_then(|length| self.bytes.len().checked_add(length))
            .ok_or_else(invalid_command)?;
        if framed_length > self.max_bytes {
            return Err(invalid_command());
        }
        self.bytes.extend_from_slice(&tag.to_be_bytes());
        self.bytes.extend_from_slice(&length.to_be_bytes());
        self.bytes.extend_from_slice(value);
        self.last_tag = tag;
        Ok(())
    }

    pub(crate) fn string(&mut self, tag: u16, value: &str) -> Result<()> {
        self.bytes(tag, value.as_bytes())
    }

    pub(crate) fn optional_string(&mut self, tag: u16, value: Option<&str>) -> Result<()> {
        let mut encoded = Vec::with_capacity(value.map_or(1, |value| value.len() + 1));
        match value {
            Some(value) => {
                encoded.push(1);
                encoded.extend_from_slice(value.as_bytes());
            }
            None => encoded.push(0),
        }
        self.bytes(tag, &encoded)
    }

    pub(crate) fn u32(&mut self, tag: u16, value: u32) -> Result<()> {
        self.bytes(tag, &value.to_be_bytes())
    }

    pub(crate) fn u64(&mut self, tag: u16, value: u64) -> Result<()> {
        self.bytes(tag, &value.to_be_bytes())
    }

    pub(crate) fn bool(&mut self, tag: u16, value: bool) -> Result<()> {
        self.bytes(tag, &[u8::from(value)])
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    const COMMAND_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
    const DEPENDENCY_A: &str = "123e4567-e89b-12d3-a456-426614174001";
    const DEPENDENCY_B: &str = "123e4567-e89b-12d3-a456-426614174002";

    #[derive(Clone)]
    struct FixturePayload {
        text: String,
        sequence: u32,
        enabled: bool,
    }

    impl CommandPayloadCodec for FixturePayload {
        const OPERATION_KIND: &'static str = "test.replace_text";
        const PAYLOAD_VERSION: u32 = 1;

        fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
            fields.string(1, &self.text)?;
            fields.u32(2, self.sequence)?;
            fields.bool(3, self.enabled)
        }
    }

    #[derive(Clone)]
    struct FixtureRevisionGuard(u64);

    impl CommandGuardCodec for FixtureRevisionGuard {
        const GUARD_KIND: &'static str = "test.revision";
        const GUARD_VERSION: u32 = 1;

        fn encode_guard(&self, fields: &mut CanonicalFields) -> Result<()> {
            fields.u64(1, self.0)
        }
    }

    #[derive(Clone)]
    struct FixtureHeadGuard(String);

    impl CommandGuardCodec for FixtureHeadGuard {
        const GUARD_KIND: &'static str = "test.head";
        const GUARD_VERSION: u32 = 2;

        fn encode_guard(&self, fields: &mut CanonicalFields) -> Result<()> {
            fields.string(1, &self.0)
        }
    }

    fn fixture_draft(command_id: &str) -> CommandDraft<FixturePayload> {
        CommandDraft {
            command_id: command_id.into(),
            account_id: "account-δ".into(),
            authorization_epoch: "7".into(),
            target: CommandTarget::new(
                CommandTargetKind::PullRequest,
                "pull-雪",
                Some("repository-1".into()),
            )
            .unwrap(),
            payload: FixturePayload {
                text: "Review ✅".into(),
                sequence: 42,
                enabled: true,
            },
            guards: vec![
                seal_guard(&FixtureRevisionGuard(91)).unwrap(),
                seal_guard(&FixtureHeadGuard("abc123".into())).unwrap(),
            ],
            dependencies: vec![DEPENDENCY_A.into(), DEPENDENCY_B.into()],
        }
    }

    fn fixture(command_id: &str) -> CommandSubmission {
        seal_command(fixture_draft(command_id)).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn canonical_envelope_and_domain_separated_hash_have_stable_golden_vectors() {
        let command = fixture(COMMAND_ID);
        assert_eq!(
            hex(command.canonical_envelope()),
            "0001000000040000000100020000000a6163636f756e742dceb40003000000080000000000000007000400000011746573742e7265706c6163655f74657874000500000004000000010006000000280001000000010200020000000870756c6c2de99baa00030000000d017265706f7369746f72792d3100070000002100010000000a52657669657720e29c850002000000040000002a0003000000010100080000006800010000003100010000000d746573742e7265766973696f6e0002000000040000000100030000000e000100000008000000000000005b00020000002b000100000009746573742e686561640002000000040000000200030000000c00010000000661626331323300090000005400010000002431323365343536372d653839622d313264332d613435362d34323636313431373430303100020000002431323365343536372d653839622d313264332d613435362d343236363134313734303032"
        );
        assert_eq!(
            command.submission_hash_hex(),
            "296ca552423cb876301290dd9e034acd37c6a7392ebb43b90b3ed77eca9b2c0f"
        );
        assert_eq!(command.submission_hash().len(), 32);
    }

    #[test]
    fn command_uuid_is_a_key_while_all_submission_facts_are_hashed() {
        let first = fixture(COMMAND_ID);
        let second = fixture("123e4567-e89b-12d3-a456-426614174099");
        assert_eq!(first.canonical_envelope(), second.canonical_envelope());
        assert_eq!(first.submission_hash(), second.submission_hash());
        assert!(!first.is_exact_retry_of(&second));

        let exact = fixture(COMMAND_ID);
        assert!(first.is_exact_retry_of(&exact));

        let mut changed_target = exact.clone();
        changed_target.target.id = "pull-other".into();
        assert_eq!(first.submission_hash(), changed_target.submission_hash());
        assert_eq!(
            first.canonical_envelope(),
            changed_target.canonical_envelope()
        );
        assert!(!first.is_exact_retry_of(&changed_target));

        let mut changed_bytes = exact;
        changed_bytes.canonical_envelope.push(0);
        assert_eq!(first.submission_hash(), changed_bytes.submission_hash());
        assert!(!first.is_exact_retry_of(&changed_bytes));
    }

    struct FixturePayloadV2(FixturePayload);

    impl CommandPayloadCodec for FixturePayloadV2 {
        const OPERATION_KIND: &'static str = "test.replace_text";
        const PAYLOAD_VERSION: u32 = 2;

        fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
            self.0.encode_payload(fields)
        }
    }

    #[test]
    fn every_immutable_submission_fact_changes_the_bytes_and_hash() {
        let original = fixture(COMMAND_ID);
        let mut variants = Vec::new();

        let mut account = fixture_draft(COMMAND_ID);
        account.account_id = "other-account".into();
        variants.push(seal_command(account).unwrap());

        let mut epoch = fixture_draft(COMMAND_ID);
        epoch.authorization_epoch = "8".into();
        variants.push(seal_command(epoch).unwrap());

        let mut target = fixture_draft(COMMAND_ID);
        target.target = CommandTarget::new(CommandTargetKind::Issue, "pull-雪", None).unwrap();
        variants.push(seal_command(target).unwrap());

        let mut payload = fixture_draft(COMMAND_ID);
        payload.payload.text.push('!');
        variants.push(seal_command(payload).unwrap());

        let mut guard = fixture_draft(COMMAND_ID);
        guard.guards[0] = seal_guard(&FixtureRevisionGuard(92)).unwrap();
        variants.push(seal_command(guard).unwrap());

        let mut dependencies = fixture_draft(COMMAND_ID);
        dependencies.dependencies.reverse();
        variants.push(seal_command(dependencies).unwrap());

        let version = fixture_draft(COMMAND_ID);
        variants.push(
            seal_command(CommandDraft {
                command_id: version.command_id,
                account_id: version.account_id,
                authorization_epoch: version.authorization_epoch,
                target: version.target,
                payload: FixturePayloadV2(version.payload),
                guards: version.guards,
                dependencies: version.dependencies,
            })
            .unwrap(),
        );

        for variant in variants {
            assert_ne!(original.canonical_envelope(), variant.canonical_envelope());
            assert_ne!(original.submission_hash(), variant.submission_hash());
            assert!(!original.is_exact_retry_of(&variant));
        }
    }

    #[test]
    fn canonical_uuid_epoch_target_and_codec_identity_fail_closed() {
        for command_id in [
            "123E4567-E89B-12D3-A456-426614174000",
            "123e4567e89b12d3a456426614174000",
            "not-a-uuid",
        ] {
            let error = seal_command(CommandDraft {
                command_id: command_id.into(),
                account_id: "account".into(),
                authorization_epoch: "1".into(),
                target: CommandTarget::new(CommandTargetKind::Issue, "issue", None).unwrap(),
                payload: FixturePayload {
                    text: String::new(),
                    sequence: 0,
                    enabled: false,
                },
                guards: vec![],
                dependencies: vec![],
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidInput);
        }

        for epoch in ["", "0", "01", "-1", "9223372036854775808"] {
            let mut draft = draft_with_dependencies(vec![]);
            draft.authorization_epoch = epoch.into();
            assert_eq!(
                seal_command(draft).unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
        assert!(CommandTarget::new(CommandTargetKind::Issue, "", None).is_err());
        assert!(CommandTarget::new(CommandTargetKind::Issue, "bad\0id", None).is_err());
        assert!(
            CommandTarget::new(
                CommandTargetKind::Issue,
                "issue",
                Some("x".repeat(MAX_COMMAND_IDENTIFIER_BYTES + 1))
            )
            .is_err()
        );
    }

    fn draft_with_dependencies(dependencies: Vec<String>) -> CommandDraft<FixturePayload> {
        CommandDraft {
            command_id: COMMAND_ID.into(),
            account_id: "account".into(),
            authorization_epoch: "1".into(),
            target: CommandTarget::new(CommandTargetKind::Issue, "issue", None).unwrap(),
            payload: FixturePayload {
                text: "text".into(),
                sequence: 1,
                enabled: false,
            },
            guards: vec![],
            dependencies,
        }
    }

    #[test]
    fn dependencies_preserve_order_and_reject_duplicates_self_edges_and_overflow() {
        let first = seal_command(draft_with_dependencies(vec![
            DEPENDENCY_A.into(),
            DEPENDENCY_B.into(),
        ]))
        .unwrap();
        let reversed = seal_command(draft_with_dependencies(vec![
            DEPENDENCY_B.into(),
            DEPENDENCY_A.into(),
        ]))
        .unwrap();
        assert_ne!(first.canonical_envelope(), reversed.canonical_envelope());
        assert_ne!(first.submission_hash(), reversed.submission_hash());

        for dependencies in [
            vec![DEPENDENCY_A.into(), DEPENDENCY_A.into()],
            vec![COMMAND_ID.into()],
            vec!["NOT-CANONICAL".into()],
        ] {
            assert_eq!(
                seal_command(draft_with_dependencies(dependencies))
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidInput
            );
        }
        let too_many = (0..=MAX_COMMAND_DEPENDENCIES)
            .map(|index| format!("00000000-0000-4000-8000-{index:012x}"))
            .collect();
        assert_eq!(
            seal_command(draft_with_dependencies(too_many))
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }

    struct BlobPayload(Vec<u8>);

    impl CommandPayloadCodec for BlobPayload {
        const OPERATION_KIND: &'static str = "test.blob";
        const PAYLOAD_VERSION: u32 = 1;

        fn encode_payload(&self, fields: &mut CanonicalFields) -> Result<()> {
            fields.bytes(1, &self.0)
        }
    }

    struct InvalidKindPayload;

    impl CommandPayloadCodec for InvalidKindPayload {
        const OPERATION_KIND: &'static str = "Test.Invalid";
        const PAYLOAD_VERSION: u32 = 1;

        fn encode_payload(&self, _: &mut CanonicalFields) -> Result<()> {
            Ok(())
        }
    }

    struct ZeroVersionPayload;

    impl CommandPayloadCodec for ZeroVersionPayload {
        const OPERATION_KIND: &'static str = "test.invalid_version";
        const PAYLOAD_VERSION: u32 = 0;

        fn encode_payload(&self, _: &mut CanonicalFields) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn payload_guard_and_envelope_bounds_are_enforced_before_admission() {
        let oversized_payload = seal_command(CommandDraft {
            command_id: COMMAND_ID.into(),
            account_id: "account".into(),
            authorization_epoch: "1".into(),
            target: CommandTarget::new(CommandTargetKind::Repository, "repo", None).unwrap(),
            payload: BlobPayload(vec![0; MAX_COMMAND_PAYLOAD_BYTES]),
            guards: vec![],
            dependencies: vec![],
        })
        .unwrap_err();
        assert_eq!(oversized_payload.code, ErrorCode::InvalidInput);

        let guard = seal_guard(&FixtureHeadGuard("x".repeat(9_000))).unwrap();
        let too_many_guards = vec![guard; MAX_COMMAND_GUARDS + 1];
        let mut draft = draft_with_dependencies(vec![]);
        draft.guards = too_many_guards;
        assert_eq!(
            seal_command(draft).unwrap_err().code,
            ErrorCode::InvalidInput
        );

        let guard = seal_guard(&FixtureHeadGuard("x".repeat(8_190))).unwrap();
        let mut draft = draft_with_dependencies(vec![]);
        draft.guards = vec![guard; MAX_COMMAND_GUARDS];
        assert_eq!(
            seal_command(draft).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn unregistered_codec_shapes_and_versions_fail_closed() {
        let invalid_kind = seal_command(CommandDraft {
            command_id: COMMAND_ID.into(),
            account_id: "account".into(),
            authorization_epoch: "1".into(),
            target: CommandTarget::new(CommandTargetKind::Repository, "repo", None).unwrap(),
            payload: InvalidKindPayload,
            guards: vec![],
            dependencies: vec![],
        })
        .unwrap_err();
        assert_eq!(invalid_kind.code, ErrorCode::InvalidInput);

        let zero_version = seal_command(CommandDraft {
            command_id: COMMAND_ID.into(),
            account_id: "account".into(),
            authorization_epoch: "1".into(),
            target: CommandTarget::new(CommandTargetKind::Repository, "repo", None).unwrap(),
            payload: ZeroVersionPayload,
            guards: vec![],
            dependencies: vec![],
        })
        .unwrap_err();
        assert_eq!(zero_version.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn guard_and_payload_order_are_structural_and_field_tags_are_strict() {
        let mut first = draft_with_dependencies(vec![]);
        first.guards = vec![
            seal_guard(&FixtureRevisionGuard(1)).unwrap(),
            seal_guard(&FixtureHeadGuard("head".into())).unwrap(),
        ];
        let mut second = draft_with_dependencies(vec![]);
        second.guards = first.guards.iter().cloned().rev().collect();
        assert_ne!(
            seal_command(first).unwrap().canonical_envelope(),
            seal_command(second).unwrap().canonical_envelope()
        );

        let mut fields = CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES);
        fields.string(2, "second").unwrap();
        assert!(fields.string(2, "duplicate").is_err());
        assert!(fields.string(1, "out-of-order").is_err());
        assert!(
            CanonicalFields::new(MAX_COMMAND_ENVELOPE_BYTES)
                .string(0, "reserved")
                .is_err()
        );
    }
}
