-- Rebuildable detail observations remain separate from authoritative summaries.
CREATE TABLE detail_observations (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks')),
    authorization_epoch TEXT NOT NULL,
    facet_revision TEXT NOT NULL,
    body_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    value_source_json TEXT,
    observed_state TEXT NOT NULL,
    stale_at TEXT,
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TABLE detail_entries (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    id TEXT NOT NULL,
    json TEXT NOT NULL,
    last_seen_run TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,facet,id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations(account_id,subject_id,facet) ON DELETE CASCADE
);
-- These are retryable read intents, never remote-write outbox commands.
CREATE TABLE detail_demand (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);
