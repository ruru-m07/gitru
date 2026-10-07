-- Authored effects are immutable and tied to the already sealed command.
CREATE TABLE command_effects (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    submission_hash BLOB NOT NULL CHECK(typeof(submission_hash)='blob' AND length(submission_hash)=32),
    version INTEGER NOT NULL CHECK(version>0),
    patch_json TEXT NOT NULL CHECK(json_valid(patch_json) AND octet_length(patch_json)<=131072),
    PRIMARY KEY(account_id,command_id),
    FOREIGN KEY(account_id,command_id,submission_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT
);
CREATE TRIGGER command_effects_immutable BEFORE UPDATE ON command_effects
BEGIN SELECT RAISE(ABORT,'immutable command effect'); END;
CREATE TRIGGER command_effects_retained BEFORE DELETE ON command_effects
BEGIN SELECT RAISE(ABORT,'command effect history requires explicit retention policy'); END;
CREATE INDEX command_effect_targets ON commands(account_id,target_id,authorization_epoch,enqueue_order) WHERE state IN ('queued','sending','retry_wait','accepted','outcome_unknown','conflict');

-- Sparse rebuildable projection: ordinary cached items require no duplicate row.
CREATE TABLE effective_item_overrides (
    account_id TEXT NOT NULL,
    id TEXT NOT NULL,
    authorization_epoch INTEGER NOT NULL CHECK(authorization_epoch>0),
    state TEXT NOT NULL,
    json TEXT NOT NULL CHECK(json_valid(json)),
    patch_json TEXT NOT NULL CHECK(json_valid(patch_json)),
    pending_json TEXT NOT NULL CHECK(json_valid(pending_json)),
    PRIMARY KEY(account_id,id),
    FOREIGN KEY(account_id,id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE VIRTUAL TABLE effective_items_fts USING fts5(account_id UNINDEXED,id UNINDEXED,title,body);
CREATE TABLE effective_item_revisions (
    account_id TEXT NOT NULL,
    id TEXT NOT NULL,
    kind TEXT NOT NULL,
    repository_id TEXT,
    revision INTEGER NOT NULL CHECK(revision>0),
    PRIMARY KEY(account_id,id),
    FOREIGN KEY(account_id,id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE INDEX effective_scope_revision ON effective_item_revisions(account_id,kind,repository_id,revision);
CREATE VIEW effective_items AS
SELECT i.account_id,i.id,i.repository_id,i.kind,coalesce(e.state,i.state) AS state,
       i.updated_at,coalesce(e.json,i.json) AS json
FROM items i LEFT JOIN effective_item_overrides e ON e.account_id=i.account_id AND e.id=i.id
    AND e.authorization_epoch=(SELECT authorization_epoch FROM accounts a WHERE a.id=i.account_id);
