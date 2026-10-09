-- Recovery fences are independent from immutable submitted intent and state.
CREATE TABLE recovery_meta (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    generation INTEGER NOT NULL CHECK(generation>=0)
) STRICT;
INSERT INTO recovery_meta(singleton,generation) VALUES(1,0);
CREATE TRIGGER recovery_generation_monotonic BEFORE UPDATE ON recovery_meta
WHEN NEW.singleton<>OLD.singleton OR NEW.generation<=OLD.generation
BEGIN SELECT RAISE(ABORT,'recovery generation must advance'); END;
CREATE TRIGGER recovery_meta_retained BEFORE DELETE ON recovery_meta
BEGIN SELECT RAISE(ABORT,'recovery generation is durable'); END;

-- Each restore adds evidence; no generic API can delete or release quarantine.
-- Delivery must check this relation inside its attempt-claim transaction.
CREATE TABLE command_recovery_quarantine (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    submission_hash BLOB NOT NULL CHECK(typeof(submission_hash)='blob' AND length(submission_hash)=32),
    recovery_generation INTEGER NOT NULL CHECK(recovery_generation>0),
    PRIMARY KEY(account_id,command_id,recovery_generation),
    FOREIGN KEY(account_id,command_id,submission_hash)
        REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER recovery_quarantine_current BEFORE INSERT ON command_recovery_quarantine
WHEN NEW.recovery_generation<>(SELECT generation FROM recovery_meta WHERE singleton=1)
BEGIN SELECT RAISE(ABORT,'invalid command recovery generation'); END;
CREATE TRIGGER recovery_quarantine_immutable BEFORE UPDATE ON command_recovery_quarantine
BEGIN SELECT RAISE(ABORT,'immutable command recovery evidence'); END;
CREATE TRIGGER recovery_quarantine_retained BEFORE DELETE ON command_recovery_quarantine
BEGIN SELECT RAISE(ABORT,'command recovery requires explicit reconciliation'); END;

-- The claim-time native check is mandatory; enforce the same fence in SQLite
-- so a future worker cannot accidentally dispatch a restored queued command.
-- Existing attempts remain immutable evidence and are not rewritten by restore.
CREATE TRIGGER quarantined_attempt_refused BEFORE INSERT ON delivery_attempts
WHEN EXISTS(SELECT 1 FROM command_recovery_quarantine q WHERE q.account_id=NEW.account_id AND q.command_id=NEW.command_id)
BEGIN SELECT RAISE(ABORT,'restored command requires reconciliation before dispatch'); END;
