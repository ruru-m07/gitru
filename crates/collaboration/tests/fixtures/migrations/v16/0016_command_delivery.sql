-- Scheduling is separate from immutable submitted intent and provider cache.
CREATE TABLE command_delivery (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    generation INTEGER NOT NULL DEFAULT 1 CHECK(generation>0),
    next_action_at TEXT,
    reconciliation_count INTEGER NOT NULL DEFAULT 0 CHECK(reconciliation_count BETWEEN 0 AND 8),
    attention TEXT CHECK(attention IN ('attempt_limit','reconciliation_limit','age_limit','evidence_limit')),
    PRIMARY KEY(account_id,command_id),
    FOREIGN KEY(account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT
) STRICT;
INSERT INTO command_delivery(account_id,command_id) SELECT account_id,command_id FROM commands;
CREATE TRIGGER command_delivery_admitted AFTER INSERT ON commands
BEGIN INSERT INTO command_delivery(account_id,command_id) VALUES(NEW.account_id,NEW.command_id); END;
CREATE TRIGGER command_delivery_identity BEFORE UPDATE OF account_id,command_id ON command_delivery
BEGIN SELECT RAISE(ABORT,'immutable delivery identity'); END;
CREATE TRIGGER command_delivery_retained BEFORE DELETE ON command_delivery
BEGIN SELECT RAISE(ABORT,'delivery scheduling requires explicit retention'); END;
CREATE TRIGGER command_delivery_generation BEFORE UPDATE ON command_delivery
WHEN NEW.generation<=OLD.generation
BEGIN SELECT RAISE(ABORT,'delivery generation must advance'); END;

-- Each attempt keeps its own execution base; submitted guards never change.
CREATE TABLE delivery_attempt_context (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL CHECK(attempt_number BETWEEN 1 AND 8),
    instance_id TEXT NOT NULL CHECK(octet_length(instance_id) BETWEEN 1 AND 4096),
    execution_base BLOB NOT NULL CHECK(typeof(execution_base)='blob' AND length(execution_base)<=65536),
    PRIMARY KEY(account_id,command_id,attempt_number),
    FOREIGN KEY(account_id,command_id,attempt_number)
        REFERENCES delivery_attempts(account_id,command_id,attempt_number) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER delivery_context_immutable BEFORE UPDATE ON delivery_attempt_context
BEGIN SELECT RAISE(ABORT,'immutable attempt execution context'); END;
CREATE TRIGGER delivery_context_retained BEFORE DELETE ON delivery_attempt_context
BEGIN SELECT RAISE(ABORT,'attempt context requires explicit retention'); END;

-- Resolution provenance is separate from the mutable scheduling/state mirror.
-- It points to an operation-specific receipt, never a generic HTTP status.
CREATE TABLE delivery_resolutions (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    delivery_generation INTEGER NOT NULL CHECK(delivery_generation>0),
    evidence_ordinal INTEGER NOT NULL CHECK(evidence_ordinal BETWEEN 0 AND 127),
    purpose TEXT NOT NULL CHECK(purpose IN ('accepted','confirmed','rejected','conflict','safe_retry')),
    PRIMARY KEY(account_id,command_id,delivery_generation),
    FOREIGN KEY(account_id,command_id,evidence_ordinal)
        REFERENCES command_evidence(account_id,command_id,ordinal) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER delivery_resolution_immutable BEFORE UPDATE ON delivery_resolutions
BEGIN SELECT RAISE(ABORT,'immutable operation resolution'); END;
CREATE TRIGGER delivery_resolution_retained BEFORE DELETE ON delivery_resolutions
BEGIN SELECT RAISE(ABORT,'operation resolution requires explicit retention'); END;

-- Keyset scans visit a bounded pending slice of one active account. Terminal
-- history never participates; the runtime rotates accounts between slices.
CREATE INDEX command_delivery_pending ON commands(account_id,command_id)
WHERE state IN ('queued','sending','retry_wait','accepted','outcome_unknown');
CREATE INDEX command_delivery_target_order ON commands(account_id,target_kind,target_id,enqueue_order)
WHERE state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict');
CREATE INDEX command_delivery_active_accounts ON accounts(id) WHERE state='active';
