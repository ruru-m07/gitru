-- Authored immutable intent has no foreign key to rebuildable provider cache.
CREATE TABLE commands (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL CHECK (length(command_id) = 36),
    authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch > 0),
    envelope_version INTEGER NOT NULL CHECK (envelope_version > 0),
    operation_kind TEXT NOT NULL CHECK (length(operation_kind) BETWEEN 1 AND 128),
    payload_version INTEGER NOT NULL CHECK (payload_version > 0),
    target_kind TEXT NOT NULL CHECK (target_kind IN ('repository','pull_request','issue','notification')),
    target_id TEXT NOT NULL CHECK (octet_length(target_id) BETWEEN 1 AND 1024),
    repository_id TEXT CHECK (repository_id IS NULL OR octet_length(repository_id) BETWEEN 1 AND 1024),
    canonical_envelope BLOB NOT NULL CHECK (typeof(canonical_envelope) = 'blob' AND length(canonical_envelope) BETWEEN 1 AND 262144),
    payload_bytes BLOB NOT NULL CHECK (typeof(payload_bytes) = 'blob' AND length(payload_bytes) <= 262144),
    guard_bytes BLOB NOT NULL CHECK (typeof(guard_bytes) = 'blob' AND length(guard_bytes) <= 262144),
    submission_hash BLOB NOT NULL CHECK (typeof(submission_hash) = 'blob' AND length(submission_hash) = 32),
    enqueue_order INTEGER NOT NULL CHECK (enqueue_order > 0),
    admitted_revision INTEGER NOT NULL CHECK (admitted_revision > 0),
    admitted_at TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued','sending','retry_wait','accepted','confirmed','outcome_unknown','conflict','rejected','cancelled','superseded')),
    PRIMARY KEY (account_id,command_id),
    UNIQUE (account_id,enqueue_order),
    UNIQUE (account_id,command_id,submission_hash),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);
CREATE INDEX command_dispatch_order ON commands(account_id,state,enqueue_order);
CREATE TRIGGER commands_immutable BEFORE UPDATE OF account_id,command_id,authorization_epoch,envelope_version,operation_kind,payload_version,target_kind,target_id,repository_id,canonical_envelope,payload_bytes,guard_bytes,submission_hash,enqueue_order,admitted_revision,admitted_at ON commands
BEGIN SELECT RAISE(ABORT,'immutable command intent'); END;
CREATE TRIGGER commands_retained BEFORE DELETE ON commands
BEGIN SELECT RAISE(ABORT,'command history requires explicit retention policy'); END;

CREATE TABLE command_dependencies (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 63),
    predecessor_id TEXT NOT NULL CHECK (predecessor_id <> command_id),
    predecessor_hash BLOB NOT NULL CHECK (typeof(predecessor_hash) = 'blob' AND length(predecessor_hash) = 32),
    required INTEGER NOT NULL DEFAULT 1 CHECK (required IN (0,1)),
    PRIMARY KEY (account_id,command_id,ordinal),
    UNIQUE (account_id,command_id,predecessor_id),
    FOREIGN KEY (account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT,
    FOREIGN KEY (account_id,predecessor_id,predecessor_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT
);
CREATE INDEX command_predecessors ON command_dependencies(account_id,predecessor_id);
CREATE INDEX command_required_predecessors ON command_dependencies(account_id,predecessor_id) WHERE required=1;
CREATE TRIGGER dependency_order BEFORE INSERT ON command_dependencies
WHEN NEW.ordinal <> (SELECT count(*) FROM command_dependencies WHERE account_id=NEW.account_id AND command_id=NEW.command_id)
 OR (SELECT enqueue_order FROM commands WHERE account_id=NEW.account_id AND command_id=NEW.predecessor_id) >= (SELECT enqueue_order FROM commands WHERE account_id=NEW.account_id AND command_id=NEW.command_id)
BEGIN SELECT RAISE(ABORT,'invalid predecessor order'); END;
CREATE TRIGGER dependencies_immutable BEFORE UPDATE OF account_id,command_id,ordinal,predecessor_id,predecessor_hash ON command_dependencies
BEGIN SELECT RAISE(ABORT,'immutable command dependency'); END;
CREATE TRIGGER dependencies_retained BEFORE DELETE ON command_dependencies
BEGIN SELECT RAISE(ABORT,'immutable command dependency'); END;

-- Dispatch APIs and state transitions land in RURU-115. Admission leaves this empty.
CREATE TABLE delivery_attempts (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch > 0),
    started_at TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('started','accepted','confirmed','rejected','outcome_unknown')),
    completed_at TEXT,
    PRIMARY KEY (account_id,command_id,attempt_number),
    FOREIGN KEY (account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT
);
CREATE TRIGGER attempt_order BEFORE INSERT ON delivery_attempts
WHEN NEW.attempt_number <> coalesce((SELECT max(attempt_number)+1 FROM delivery_attempts WHERE account_id=NEW.account_id AND command_id=NEW.command_id),1)
BEGIN SELECT RAISE(ABORT,'invalid attempt order'); END;
CREATE TRIGGER attempt_identity_immutable BEFORE UPDATE OF account_id,command_id,attempt_number,authorization_epoch,started_at ON delivery_attempts
BEGIN SELECT RAISE(ABORT,'immutable attempt identity'); END;
CREATE TRIGGER attempts_retained BEFORE DELETE ON delivery_attempts
BEGIN SELECT RAISE(ABORT,'attempt evidence requires explicit retention policy'); END;

CREATE TABLE command_evidence (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 127),
    attempt_number INTEGER,
    kind TEXT NOT NULL CHECK (octet_length(kind) BETWEEN 1 AND 128),
    version INTEGER NOT NULL CHECK (version > 0),
    payload BLOB NOT NULL CHECK (typeof(payload) = 'blob' AND length(payload) <= 65536),
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (account_id,command_id,ordinal),
    FOREIGN KEY (account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT,
    FOREIGN KEY (account_id,command_id,attempt_number) REFERENCES delivery_attempts(account_id,command_id,attempt_number) ON DELETE RESTRICT
);
CREATE TRIGGER evidence_immutable BEFORE UPDATE ON command_evidence
BEGIN SELECT RAISE(ABORT,'immutable command evidence'); END;
CREATE TRIGGER evidence_retained BEFORE DELETE ON command_evidence
BEGIN SELECT RAISE(ABORT,'command evidence requires explicit retention policy'); END;

CREATE TABLE command_target_protections (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    reference_kind TEXT NOT NULL CHECK (reference_kind IN ('entity','facet','blob')),
    reference_id TEXT NOT NULL CHECK (octet_length(reference_id) BETWEEN 1 AND 1024),
    facet TEXT NOT NULL DEFAULT '' CHECK ((reference_kind='facet' AND octet_length(facet) BETWEEN 1 AND 128) OR (reference_kind<>'facet' AND facet='')),
    required INTEGER NOT NULL DEFAULT 1 CHECK (required IN (0,1)),
    PRIMARY KEY (account_id,command_id,reference_kind,reference_id,facet),
    FOREIGN KEY (account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT
);
CREATE INDEX command_protection_lookup ON command_target_protections(account_id,reference_id,reference_kind,facet) WHERE required=1;
CREATE TRIGGER protection_bound BEFORE INSERT ON command_target_protections
WHEN (SELECT count(*) FROM command_target_protections WHERE account_id=NEW.account_id AND command_id=NEW.command_id) >= 128
BEGIN SELECT RAISE(ABORT,'too many command protections'); END;
CREATE TRIGGER protection_identity_immutable BEFORE UPDATE OF account_id,command_id,reference_kind,reference_id,facet ON command_target_protections
BEGIN SELECT RAISE(ABORT,'immutable command protection'); END;
CREATE TRIGGER protections_retained BEFORE DELETE ON command_target_protections
BEGIN SELECT RAISE(ABORT,'command protection requires explicit retention policy'); END;


CREATE TRIGGER dependency_required_valid BEFORE UPDATE OF required ON command_dependencies
WHEN NEW.required <> EXISTS(SELECT 1 FROM commands c WHERE c.account_id=NEW.account_id AND c.command_id=NEW.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict'))
BEGIN SELECT RAISE(ABORT,'invalid dependency requirement'); END;
CREATE TRIGGER protection_required_valid BEFORE UPDATE OF required ON command_target_protections
WHEN NEW.required <> (EXISTS(SELECT 1 FROM commands c WHERE c.account_id=NEW.account_id AND c.command_id=NEW.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) OR EXISTS(SELECT 1 FROM command_dependencies d WHERE d.account_id=NEW.account_id AND d.predecessor_id=NEW.command_id AND d.required=1))
BEGIN SELECT RAISE(ABORT,'invalid protection requirement'); END;

-- These two flags are derived local indexes, not immutable submitted intent.
-- Their partial indexes keep retention independent of terminal receipt history.
CREATE TRIGGER dependency_requirement AFTER INSERT ON command_dependencies
BEGIN
    UPDATE command_dependencies SET required=EXISTS(SELECT 1 FROM commands c WHERE c.account_id=NEW.account_id AND c.command_id=NEW.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict'))
    WHERE account_id=NEW.account_id AND command_id=NEW.command_id AND ordinal=NEW.ordinal;
    UPDATE command_target_protections SET required=(EXISTS(SELECT 1 FROM commands c WHERE c.account_id=NEW.account_id AND c.command_id=NEW.predecessor_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) OR EXISTS(SELECT 1 FROM command_dependencies d WHERE d.account_id=NEW.account_id AND d.predecessor_id=NEW.predecessor_id AND d.required=1))
    WHERE account_id=NEW.account_id AND command_id=NEW.predecessor_id;
END;
CREATE TRIGGER protection_requirement AFTER INSERT ON command_target_protections
BEGIN
    UPDATE command_target_protections SET required=(EXISTS(SELECT 1 FROM commands c WHERE c.account_id=NEW.account_id AND c.command_id=NEW.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) OR EXISTS(SELECT 1 FROM command_dependencies d WHERE d.account_id=NEW.account_id AND d.predecessor_id=NEW.command_id AND d.required=1))
    WHERE account_id=NEW.account_id AND command_id=NEW.command_id AND reference_kind=NEW.reference_kind AND reference_id=NEW.reference_id AND facet=NEW.facet;
END;
CREATE TRIGGER command_requirement AFTER UPDATE OF state ON commands
BEGIN
    UPDATE command_dependencies SET required=NEW.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')
    WHERE account_id=NEW.account_id AND command_id=NEW.command_id;
    UPDATE command_target_protections SET required=(EXISTS(SELECT 1 FROM commands c WHERE c.account_id=command_target_protections.account_id AND c.command_id=command_target_protections.command_id AND c.state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict')) OR EXISTS(SELECT 1 FROM command_dependencies d WHERE d.account_id=command_target_protections.account_id AND d.predecessor_id=command_target_protections.command_id AND d.required=1))
    WHERE account_id=NEW.account_id AND (command_id=NEW.command_id OR command_id IN (SELECT predecessor_id FROM command_dependencies WHERE account_id=NEW.account_id AND command_id=NEW.command_id));
END;
