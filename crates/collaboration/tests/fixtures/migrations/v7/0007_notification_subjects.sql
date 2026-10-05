-- Locators observed in the same accepted inbox page; retained rows are not grants.
CREATE TABLE notification_subject_selectors (
    account_id TEXT NOT NULL,
    notification_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    authorization_epoch TEXT NOT NULL,
    selector_generation TEXT NOT NULL,
    mapping_json TEXT NOT NULL,
    kind TEXT,
    repository_provider_id TEXT,
    number TEXT,
    PRIMARY KEY(account_id,notification_id),
    FOREIGN KEY(account_id,notification_id) REFERENCES items(account_id,id) ON DELETE CASCADE,
    FOREIGN KEY(account_id,instance_id) REFERENCES account_instances(account_id,instance_id) ON DELETE RESTRICT
);
CREATE INDEX notification_subject_coordinates ON notification_subject_selectors(account_id,instance_id,kind,repository_provider_id,number);

-- Rebuildable bounded explicit GET-only intent, never a remote write/outbox.
CREATE TABLE notification_subject_discovery (
    account_id TEXT NOT NULL,
    notification_id TEXT NOT NULL,
    authorization_epoch TEXT NOT NULL,
    selector_generation TEXT NOT NULL,
    intent_generation TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3),
    run_id TEXT,
    outcome_reason TEXT,
    PRIMARY KEY(account_id,notification_id),
    FOREIGN KEY(account_id,notification_id) REFERENCES notification_subject_selectors(account_id,notification_id) ON DELETE CASCADE
);
CREATE INDEX notification_subject_pending ON notification_subject_discovery(requested,account_id,notification_id);

-- Point reads examine one canonical target's immutable aliases and pending
-- representations; avoid scans of an account's entire retained alias catalog.
CREATE INDEX native_alias_target ON resource_aliases(account_id,instance_id,kind,entity_id,alias_kind);
CREATE INDEX pending_native_representation ON pending_endpoint_aliases(account_id,instance_id,kind,native_identity);
