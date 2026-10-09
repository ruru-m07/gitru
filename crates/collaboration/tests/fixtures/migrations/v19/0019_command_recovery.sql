-- User decisions are separate from immutable original intent and provider proof.
CREATE TABLE command_user_controls (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    paused INTEGER NOT NULL DEFAULT 0 CHECK(paused IN (0,1)),
    PRIMARY KEY(account_id,command_id),
    FOREIGN KEY(account_id,command_id) REFERENCES commands(account_id,command_id) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER command_controls_identity BEFORE UPDATE OF account_id,command_id ON command_user_controls
BEGIN SELECT RAISE(ABORT,'immutable command control identity'); END;
CREATE TRIGGER command_controls_retained BEFORE DELETE ON command_user_controls
BEGIN SELECT RAISE(ABORT,'command controls require explicit retention'); END;

CREATE TABLE command_recovery_actions (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    action_id TEXT NOT NULL CHECK(length(action_id)=36),
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 63),
    submission_hash BLOB NOT NULL CHECK(typeof(submission_hash)='blob' AND length(submission_hash)=32),
    request_json TEXT NOT NULL CHECK(json_valid(request_json) AND octet_length(request_json)<=262144),
    receipt_json TEXT NOT NULL CHECK(json_valid(receipt_json) AND octet_length(receipt_json)<=16384),
    PRIMARY KEY(account_id,action_id),
    UNIQUE(account_id,command_id,ordinal),
    FOREIGN KEY(account_id,command_id,submission_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT,
    CHECK(json_extract(request_json,'$.context.account_id') IS account_id AND json_extract(request_json,'$.context.command_id') IS command_id AND json_extract(request_json,'$.action_id') IS action_id),
    CHECK(json_extract(receipt_json,'$.account_id') IS account_id AND json_extract(receipt_json,'$.command_id') IS command_id AND json_extract(receipt_json,'$.action_id') IS action_id)
) STRICT;
CREATE TRIGGER command_recovery_action_bound BEFORE INSERT ON command_recovery_actions
WHEN NEW.ordinal<>(SELECT count(*) FROM command_recovery_actions WHERE account_id=NEW.account_id AND command_id=NEW.command_id)
 OR coalesce((SELECT sum(octet_length(request_json)) FROM command_recovery_actions WHERE account_id=NEW.account_id AND command_id=NEW.command_id),0)+octet_length(NEW.request_json)>1048576
BEGIN SELECT RAISE(ABORT,'invalid recovery action history'); END;
CREATE TRIGGER command_recovery_action_immutable BEFORE UPDATE ON command_recovery_actions
BEGIN SELECT RAISE(ABORT,'immutable recovery action'); END;
CREATE TRIGGER command_recovery_action_retained BEFORE DELETE ON command_recovery_actions
BEGIN SELECT RAISE(ABORT,'recovery actions require explicit retention'); END;

-- No dependency is rewritten. A replacement inherits only the original FIFO
-- position; successors still require their original exact predecessor proof.
CREATE TABLE command_supersessions (
    account_id TEXT NOT NULL,
    original_id TEXT NOT NULL,
    original_hash BLOB NOT NULL CHECK(typeof(original_hash)='blob' AND length(original_hash)=32),
    replacement_id TEXT NOT NULL CHECK(replacement_id<>original_id),
    replacement_hash BLOB NOT NULL CHECK(typeof(replacement_hash)='blob' AND length(replacement_hash)=32),
    action_id TEXT NOT NULL,
    execution_order INTEGER NOT NULL CHECK(execution_order>0),
    depth INTEGER NOT NULL CHECK(depth BETWEEN 1 AND 16),
    PRIMARY KEY(account_id,original_id),
    UNIQUE(account_id,replacement_id),
    UNIQUE(account_id,action_id),
    FOREIGN KEY(account_id,original_id,original_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,replacement_id,replacement_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,action_id) REFERENCES command_recovery_actions(account_id,action_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TRIGGER command_supersession_valid BEFORE INSERT ON command_supersessions
WHEN NOT EXISTS(
 SELECT 1 FROM commands old JOIN commands fresh ON fresh.account_id=old.account_id
 JOIN accounts a ON a.id=old.account_id
 WHERE old.account_id=NEW.account_id AND old.command_id=NEW.original_id AND fresh.command_id=NEW.replacement_id
 AND old.authorization_epoch=fresh.authorization_epoch AND a.authorization_epoch=old.authorization_epoch AND a.state='active'
 AND old.target_kind=fresh.target_kind AND old.target_id=fresh.target_id AND old.repository_id IS fresh.repository_id
 AND old.operation_kind=fresh.operation_kind AND old.payload_version=fresh.payload_version
 AND old.enqueue_order<fresh.enqueue_order AND old.state='superseded' AND fresh.state='queued'
 AND NEW.execution_order=coalesce((SELECT execution_order FROM command_supersessions WHERE account_id=NEW.account_id AND replacement_id=NEW.original_id),old.enqueue_order)
 AND NEW.depth=coalesce((SELECT depth FROM command_supersessions WHERE account_id=NEW.account_id AND replacement_id=NEW.original_id),0)+1
) OR EXISTS(SELECT 1 FROM command_recovery_quarantine WHERE account_id=NEW.account_id AND command_id IN (NEW.original_id,NEW.replacement_id))
 OR EXISTS(SELECT 1 FROM delivery_attempts WHERE account_id=NEW.account_id AND command_id=NEW.replacement_id)
 OR EXISTS(SELECT 1 FROM command_dependencies d JOIN commands p ON p.account_id=d.account_id AND p.command_id=d.predecessor_id LEFT JOIN command_supersessions s ON s.account_id=p.account_id AND s.replacement_id=p.command_id WHERE d.account_id=NEW.account_id AND d.command_id=NEW.replacement_id AND coalesce(s.execution_order,p.enqueue_order)>=NEW.execution_order)
BEGIN SELECT RAISE(ABORT,'invalid command supersession'); END;
CREATE TRIGGER command_supersession_immutable BEFORE UPDATE ON command_supersessions
BEGIN SELECT RAISE(ABORT,'immutable command supersession'); END;
CREATE TRIGGER command_supersession_retained BEFORE DELETE ON command_supersessions
BEGIN SELECT RAISE(ABORT,'command supersessions require explicit retention'); END;
CREATE TRIGGER command_recovery_action_supersession AFTER INSERT ON command_recovery_actions
WHEN (json_extract(NEW.receipt_json,'$.replacement_id') IS NOT NULL AND NOT EXISTS(
 SELECT 1 FROM command_supersessions s WHERE s.account_id=NEW.account_id AND s.original_id=NEW.command_id AND s.action_id=NEW.action_id
 AND s.replacement_id IS json_extract(NEW.request_json,'$.new_command_id') AND s.replacement_id IS json_extract(NEW.receipt_json,'$.replacement_id')
 AND json_extract(NEW.receipt_json,'$.state')='superseded'
)) OR (json_extract(NEW.receipt_json,'$.replacement_id') IS NULL AND EXISTS(SELECT 1 FROM command_supersessions WHERE account_id=NEW.account_id AND action_id=NEW.action_id))
BEGIN SELECT RAISE(ABORT,'recovery receipt does not match supersession'); END;
CREATE INDEX command_recovery_pending ON commands(account_id,enqueue_order DESC)
WHERE state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict');
CREATE INDEX command_recovery_target_history ON commands(account_id,target_id,enqueue_order DESC);
CREATE INDEX command_recovery_target_pending ON commands(account_id,target_id,enqueue_order DESC)
WHERE state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict');
